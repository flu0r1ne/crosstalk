mod repl;
mod tempfile;

use die::die;
use std::io::{self, IsTerminal, Read, Write};

use self::repl::Repl;

use crate::chat::{Message, Role};
use crate::providers::{ChatProvider, MessageDelta};
use crate::registry::populate::{populated_registry, resolve_once};
use crate::ChatArgs;

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

    pub(crate) fn public_messages(&self) -> Vec<Message> {
        self.buf
            .iter()
            .filter(|x| !matches!((*x).role, Role::Info))
            .cloned()
            .collect()
    }

    pub(crate) fn clear(&mut self) {
        self.buf.clear();
    }
}

pub(crate) struct MessageBuilder {
    msg: Option<Message>,
}

impl MessageBuilder {
    pub(crate) fn new() -> MessageBuilder {
        MessageBuilder { msg: None }
    }

    pub(crate) fn add(&mut self, delta: &MessageDelta) {
        if let Some(msg) = &mut self.msg {
            msg.content.push_str(&delta.content);
        } else {
            self.msg = Some(Message::new(Role::User, delta.content.clone()));
        }
    }
}

impl TryFrom<MessageBuilder> for Message {
    type Error = ();

    fn try_from(value: MessageBuilder) -> Result<Self, Self::Error> {
        if let Some(msg) = value.msg {
            Ok(msg)
        } else {
            Err(())
        }
    }
}

pub(crate) async fn chat_cmd(args: &ChatArgs) {
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
        die!("It appears that an initial prompt is being provided both through standard input and the prompt argument.");
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

    // Resolve the model specified in the "model" argument (or the default).
    let registry = populated_registry().await;

    let resolve_result = resolve_once(&registry, args.model.clone()).await;

    let (provider, model_id) = match resolve_result {
        Ok(resolved) => resolved,
        Err(err) => {
            die!("failed to resolve model: {}", err);
        }
    };

    // If the output is a terminal (e.g., user-facing), incrementally print it.
    let incremental = out_terminal;

    chat(
        provider,
        &model_id,
        initial_prompt,
        interactive,
        incremental,
    )
    .await;
}

async fn chat<'p>(
    provider: &'p Box<dyn ChatProvider>,
    model_id: &str,
    initial_prompt: Option<String>,
    interactive: bool,
    incremental: bool,
) {
    let mut pending_init_prompt = initial_prompt.is_some();

    // Add the initial prompt to the internal buffer.
    let mut msg_buf = MessageBuffer::new();

    if let Some(initial_prompt) = initial_prompt {
        msg_buf.add_message(Message::new(Role::User, initial_prompt));
    }

    // Only initialize the REPL if  it is really needed.
    let mut repl = if interactive {
        Some(Repl::new(None))
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

            if prompt.is_none() {
                break;
            }

            let prompt = prompt.unwrap();

            msg_buf.add_message(Message::new(Role::User, prompt));
        }

        let mut completion = provider
            .stream_completion(&model_id, &msg_buf.public_messages())
            .await
            .expect("Completion failed.");

        let mut msg_builder = MessageBuilder::new();

        if interactive {
            print!("[{}] ", model_id);
            flush_or_die();
        }

        while let Some(update) = completion.next().await {
            match update {
                Ok(delta) => {
                    if incremental {
                        print!("{}", delta.content);
                        flush_or_die();
                    }

                    msg_builder.add(&delta);
                }
                Err(err) => panic!("Failed to decode streaming response: {}", err),
            }
        }

        let msg: Message = msg_builder.try_into().unwrap();

        if incremental {
            println!("\n");
        } else {
            print!("{}", msg.content);
        }

        if !interactive {
            break;
        }

        pending_init_prompt = false;
    }
}
