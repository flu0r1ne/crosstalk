use std::env;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultCompleter, Emacs, KeyCode, KeyModifiers,
    ReedlineEvent, ReedlineMenu,
};
use reedline::{DefaultPrompt, DefaultPromptSegment, EditCommand, MenuBuilder, Reedline, Signal};

use die::die;
use nu_ansi_term::{Color, Style};

use tempfile::NamedTempFile;

use super::MessageBuffer;

/// Attempt to resolve the system editor if one is not explicitly specified.
fn resolve_system_editor() -> Option<String> {
    let fallback_editors = ["editor", "vim", "emacs", "vi", "nano"];

    if let Some(editor) = env::var("EDITOR").ok() {
        return Some(editor);
    }

    if let Some(paths) = env::var_os("PATH") {
        for path in env::split_paths(&paths) {
            for editor in &fallback_editors {
                let full_path = PathBuf::from(path.join(editor));
                if full_path.exists() {
                    return Some(editor.to_string());
                }
            }
        }
    }

    None
}

/// Launch an interactive editor with a temporary file.
fn launch_interactive_editor(editor: Option<String>) -> String {
    // Create a named temporary file
    let tmp_file = NamedTempFile::new().expect("Failed to create temporary file");

    // Resolve editor using the provided logic
    let editor = editor
        .or_else(resolve_system_editor)
        .expect("No suitable editor found");

    // Launch the editor subprocess
    let status = Command::new(&editor)
        .arg(tmp_file.path())
        .status()
        .expect("Failed to launch editor");

    if !status.success() {
        die!(
            "Error: the specified editor \"{}\" did not exit successfully.",
            editor
        );
    }

    // Read the resulting file into a string
    let mut edited_content = String::new();
    {
        let mut file = File::open(tmp_file.path()).expect("Failed to open temporary file");
        file.read_to_string(&mut edited_content)
            .expect("Failed to read temporary file");
    }

    edited_content
}

pub(crate) struct Repl {
    line_editor: Reedline,
    prompt: DefaultPrompt,
}

impl Repl {
    pub(crate) fn new() -> Repl {
        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic("[#]".to_string()),
            DefaultPromptSegment::Empty,
        );

        let commands = vec!["/edit".into(), "/exit".into(), "/clear".into()];

        let mut completer = Box::new(DefaultCompleter::with_inclusions(&['/']));

        completer.insert(commands);

        // Use the interactive menu to select options from the completer
        let completion_menu = Box::new(
            ColumnarMenu::default()
                .with_name("completion_menu")
                .with_text_style(Style::new().fg(Color::Default))
                .with_selected_text_style(Style::new().fg(Color::Blue).on(Color::DarkGray))
                .with_selected_match_text_style(
                    Style::new().fg(Color::Blue).bold().on(Color::DarkGray),
                ),
        );

        // Set up the required keybindings
        let mut keybindings = default_emacs_keybindings();

        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );

        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('j'),
            ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
        );

        let edit_mode = Box::new(Emacs::new(keybindings));

        let line_editor = Reedline::create()
            .with_completer(completer)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(edit_mode);

        Repl {
            line_editor,
            prompt,
        }
    }

    pub(crate) fn edit(&mut self, msg_buf: &mut MessageBuffer) -> Option<String> {
        loop {
            let sig = self.line_editor.read_line(&self.prompt);

            match sig {
                Ok(Signal::Success(buffer)) => {
                    match buffer.as_str() {
                        "/exit" => break,
                        "/edit" => {
                            let buffer = launch_interactive_editor(None);

                            println!("{}", buffer);

                            return Some(buffer);
                        }
                        "/clear" => {
                            msg_buf.clear();
                            continue;
                        }
                        _ => return Some(buffer),
                    };
                }
                Ok(Signal::CtrlD) | Ok(Signal::CtrlC) => {
                    break;
                }
                _ => break,
            }
        }

        None
    }
}
