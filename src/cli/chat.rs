mod highlighter;
mod prompt;
mod repl;
mod tempfile;

use crate::utils::errors::{fmt_error, fmt_warn};
use crate::{chat, die, version};

use core::fmt;
use std::error::Error;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

use self::repl::Repl;

use crate::chat::Role;
use crate::config;
use crate::providers::{ChatProvider, ContextManagement, MessageDelta};
use crate::registry::populate::resolve_once;
use crate::registry::registry::{self, ModelSpec, Registry};
use crate::ChatArgs;
use prompt::{model_prompt, user_prompt};
use tokio::{select, signal};


pub(crate) enum Severity {
    Error,
    Warn,
    Standard,
}

pub(crate) enum Message {
    Chat(chat::Message, Option<String>),
    Command(String),
    Output(Severity, String),
}

impl Message {
    pub(crate) fn warn(msg: String) -> Message {
        Message::Output(Severity::Warn, msg)
    }

    pub(crate) fn error(msg: String) -> Message {
        Message::Output(Severity::Error, msg)
    }

    pub(crate) fn output(msg: String) -> Message {
        Message::Output(Severity::Standard, msg)
    }

    pub(crate) fn command(msg: String) -> Message {
        Message::Command(msg)
    }

    pub(crate) fn user(msg: String) -> Message {
        Message::Chat(chat::Message::new(Role::User, msg), None)
    }

    pub(crate) fn model(msg: String, model_id: String) -> Message {
        Message::Chat(chat::Message::new(Role::Model, msg), Some(model_id))
    }

    pub(crate) fn system(msg: String) -> Message {
        Message::Chat(chat::Message::new(Role::System, msg), None)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Chat(message, model_id) => match &message.role {
                Role::User => write!(f, "{}{}", user_prompt(), message.content),
                Role::System => Ok(()),
                Role::Model => write!(
                    f,
                    "{}{}",
                    model_prompt(model_id.as_ref().unwrap()),
                    message.content
                ),
            },
            Message::Command(command) => {
                write!(f, "{}{}", user_prompt(), command)
            }
            Message::Output(severity, msg) => match severity {
                Severity::Warn => fmt_warn::<&str>(f, msg),
                Severity::Error => fmt_error::<&str>(f, msg),
                Severity::Standard => write!(f, "{}", msg),
            },
        }
    }
}

pub(crate) struct MessageBuffer {
    buf: Vec<Message>,
}

impl MessageBuffer {
    pub(crate) fn new() -> MessageBuffer {
        MessageBuffer {
            buf: Vec::<Message>::new(),
        }
    }

    pub(crate) fn add_message(&mut self, msg: Message) {
        self.buf.push(msg);
    }

    pub(crate) fn chat_messages(&self) -> Vec<chat::Message> {
        self.buf
            .iter()
            .filter_map(|msg| match msg {
                Message::Chat(msg, _) => Some(msg.clone()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn clear(&mut self) {
        self.buf.clear();
    }
}

pub(crate) struct MessageBuilder {
    msg: Option<chat::Message>,
}

impl MessageBuilder {
    pub(crate) fn new() -> MessageBuilder {
        MessageBuilder { msg: None }
    }

    pub(crate) fn add(&mut self, delta: &MessageDelta) {
        if let Some(msg) = &mut self.msg {
            msg.content.push_str(&delta.content);
        } else {
            self.msg = Some(chat::Message::new(Role::User, delta.content.clone()));
        }
    }
}

impl TryFrom<MessageBuilder> for chat::Message {
    type Error = ();

    fn try_from(value: MessageBuilder) -> Result<Self, Self::Error> {
        if let Some(msg) = value.msg {
            Ok(msg)
        } else {
            Err(())
        }
    }
}

pub(crate) async fn chat_cmd(
    editor: Option<PathBuf>,
    keybindings: config::Keybindings,
    default_model: Option<String>,
    registry: Registry,
    args: &ChatArgs,
) {
    let in_terminal = io::stdin().is_terminal();
    let out_terminal = io::stdout().is_terminal();

    // If standard input is a terminal and interactive mode has not been specified,
    // gather input from standard input with the assumption that we are not running interactively.
    let interactive = if args.prompt.is_some() {
        args.interactive
    } else {
        in_terminal && out_terminal
    };

    if args.prompt.is_some() && !in_terminal {
        die!("it appears that an initial prompt is being provided both through standard input and the prompt argument");
    }

    // Obtain the initial prompt, either from standard input or from a positional argument.
    let initial_prompt = if let Some(prompt) = &args.prompt {
        Some(prompt.clone())
    } else if !in_terminal {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .expect("Failed to read the initial prompt from standard input.");
        Some(buf)
    } else {
        None
    };

    let model = args.model.clone().or_else(|| default_model);

    let resolve_result = resolve_once(&registry, model).await;

    let (provider, model_id) = match resolve_result {
        Ok(resolved) => resolved,
        Err(err) => {
            // When the default model is unset or a provider is not activate, this
            // could be due to the complete absense of any provider. This is a more
            // friendly error message, since the remediation action should be obvious
            // to newcomers.
            if registry.empty() {
                die!("none of the chat providers are active, at least one needs to be active to start a chat");
            }

            die!("failed to resolve model: {}", err);
        }
    };

    // If the output is a terminal (e.g., user-facing), incrementally print it.
    let incremental = out_terminal;

    chat(
        editor,
        keybindings,
        provider,
        &model_id,
        initial_prompt,
        interactive,
        incremental,
    )
    .await;
}

async fn chat<'p>(
    editor: Option<PathBuf>,
    keybindings: config::Keybindings,
    provider: &'p Box<dyn ChatProvider>,
    model_id: &str,
    initial_prompt: Option<String>,
    interactive: bool,
    incremental: bool,
) {
    if interactive {
        println!("{} version {}", version::NAME, version::VERSION);
    }

    let mut pending_init_prompt = initial_prompt.is_some();

    let spec = ModelSpec::resolved(provider.id(), model_id.to_string());

    // Add the initial prompt to the internal buffer.
    let mut msg_buf = MessageBuffer::new();

    match provider.context_management() {
        ContextManagement::Implicit => {
            let implicit_warning = Message::warn(
                "This provider implicity manages context. The context may be truncated without warning.".to_string()
            );

            eprintln!("{}", implicit_warning);

            msg_buf.add_message(implicit_warning);
        }
        ContextManagement::Explicit => {}
    }

    if let Some(initial_prompt) = initial_prompt {
        msg_buf.add_message(Message::user(initial_prompt));
    }

    // Only initialize the REPL if  it is really needed.
    let mut repl = if interactive {
        Some(Repl::new(editor, keybindings))
    } else {
        None
    };

    let flush_or_die = || {
        std::io::stdout()
            .flush()
            .expect("Failed to flush the output stream.");
    };

    loop {
        // Prompt after the initial prompt is dispensed with.
        if !pending_init_prompt && interactive {
            let repl = repl.as_mut().unwrap();

            let prompt = repl.edit(&mut msg_buf);

            let prompt = match prompt {
                Some(prompt) => prompt,
                None => break,
            };

            msg_buf.add_message(Message::user(prompt));
        }
       
        let completion = provider
            .stream_completion(&model_id, &msg_buf.chat_messages())
            .await;

        let mut completion = match completion {
            Ok(completion) => completion,
            Err(err) => {
                let mut err_msg = format!("completion for {} failed: {}", spec, err);

                if let Some(source) = err.source() {
                    err_msg.push_str(&format!("\n{}", source));
                }

                let completion_error = Message::error(err_msg);

                eprintln!("{}", completion_error);

                msg_buf.add_message(completion_error);

                continue;
            }
        };

        let mut msg_builder = MessageBuilder::new();

        if interactive {
            let model_prompt = model_prompt(model_id);
            print!("{} ", model_prompt);
            flush_or_die();
        }

        let mut skip_response = false;

        loop {
            select! {
                update = completion.next() => {
                    let update = match update {
                        Some(update) => update,
                        None => break
                    };

                    match update {
                        Ok(delta) => {
                            if incremental {
                                print!("{}", delta.content);
                                flush_or_die();
                            }
        
                            msg_builder.add(&delta);
                        }
                        Err(err) => panic!("failed to decode streaming response: {}", err),
                    }
                }
                _ = signal::ctrl_c() => {
                    skip_response = true;
                    break;
                } 
            }
        }

        let msg: chat::Message = match msg_builder.try_into() {
            Ok(msg) => msg,
            Err(()) => continue,
        };

        if incremental {
            println!("\n");
        } else {
            print!("{}", msg.content);
        }

        if !skip_response {
            msg_buf.add_message(Message::Chat(msg, Some(model_id.to_string())));
        }

        if !interactive {
            break;
        }
 
        pending_init_prompt = false;
    }
}
