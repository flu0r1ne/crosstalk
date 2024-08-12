use std::env;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;

use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultCompleter, EditMode, Emacs, KeyCode,
    KeyModifiers, Keybindings, ReedlineEvent, ReedlineMenu,
};
use reedline::{
    default_vi_insert_keybindings, default_vi_normal_keybindings, DefaultPrompt,
    DefaultPromptSegment, EditCommand, MenuBuilder, Reedline, Signal, Vi,
};

use crate::cli::chat::Message;
use crate::die;
use crate::{config, warn};
use nu_ansi_term::{Color, Style};

use super::highlighter::Highlighter;
use super::prompt::{completion_marker, Prompt};
use super::tempfile::Tempfile;
use super::MessageBuffer;

/// Attempts to resolve the preferred editor. If the EDITOR environment variable
/// is defined, the command specified by it is used. If a Debian-specific editor
/// is specified, it is used. Otherwise, the PATH is searched for common editors,
/// and the first found editor is used.
fn resolve_fallback_editor() -> Option<PathBuf> {
    let fallback_editors = ["editor", "vim", "emacs", "vi", "nano"];

    if let Some(editor) = env::var("EDITOR").ok() {
        return Some(editor.into());
    }

    if let Some(paths) = env::var_os("PATH") {
        for path in env::split_paths(&paths) {
            for editor in &fallback_editors {
                let full_path = PathBuf::from(path.join(editor));
                if full_path.exists() {
                    return Some(full_path);
                }
            }
        }
    }

    None
}

/// Launches an interactive editor to edit the contents of a file and return the result.
/// The `editor` parameter specifies the editor to use, `temp_file` represents the
/// temporary file where initial contents are stored.
fn read_from_interactive_editor(editor: &PathBuf, temp_file: &mut Tempfile) -> String {
    // Remove the previous contents of the file
    {
        if let Err(err) = temp_file.file_mut().set_len(0) {
            die!("failed to truncate the editor file: {}", err);
        }

        if let Err(err) = temp_file.file_mut().seek(SeekFrom::Start(0)) {
            die!("failed to reset file cursor: {}", err);
        }
    }

    // Launch the editor subprocess
    let status = Command::new(editor.clone()).arg(temp_file.path()).status();

    let status = match status {
        Ok(status) => status,
        Err(err) => {
            die!("failed to launch editor: {}", err);
        }
    };

    if !status.success() {
        let program = String::from_utf8_lossy(editor.as_os_str().as_bytes());

        die!(
            "the specified editor \"{}\" did not exit successfully.",
            program
        );
    }

    // Read the resulting file into a string
    let mut edited_content = String::new();
    {
        if let Err(err) = temp_file.file_mut().read_to_string(&mut edited_content) {
            die!(
                "failed to read in the editor file: {}, was it deleted?",
                err
            );
        }
    }

    edited_content
}

fn edit_mode(keybindings: config::Keybindings) -> Box<dyn EditMode> {
    match keybindings {
        config::Keybindings::Vi => {
            let mut insert_bindings = default_vi_insert_keybindings();

            insert_bindings.add_binding(
                KeyModifiers::NONE,
                KeyCode::Tab,
                ReedlineEvent::UntilFound(vec![
                    ReedlineEvent::Menu("completion_menu".to_string()),
                    ReedlineEvent::MenuNext,
                ]),
            );

            Box::new(Vi::new(insert_bindings, default_vi_normal_keybindings()))
        }
        config::Keybindings::Emacs => {
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
                KeyCode::Char('e'),
                ReedlineEvent::OpenEditor,
            );

            keybindings.add_binding(
                KeyModifiers::CONTROL,
                KeyCode::Char('j'),
                ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
            );

            Box::new(Emacs::new(keybindings))
        }
    }
}

pub(crate) struct Repl {
    line_editor: Reedline,
    prompt: Prompt,
    tempfile: Tempfile,
    editor: Option<PathBuf>,
}

impl Repl {
    pub(crate) fn new(editor: Option<PathBuf>, keybindings: config::Keybindings) -> Repl {
        let prompt = Prompt::default();

        let tempfile =
            Tempfile::with_base_and_ext("msg", ".xtalk").expect("failed to create temporary file");

        let commands = vec!["/edit".into(), "/exit".into(), "/clear".into()];

        let mut completer = Box::new(DefaultCompleter::with_inclusions(&['/']));

        completer.insert(commands);

        // Use the interactive menu to select options from the completer
        let completion_menu = Box::new(
            ColumnarMenu::default()
                .with_name("completion_menu")
                .with_marker(&completion_marker().to_string())
                .with_text_style(Style::new().fg(Color::Default))
                .with_selected_text_style(Style::new().fg(Color::Blue).on(Color::DarkGray))
                .with_selected_match_text_style(
                    Style::new().fg(Color::Blue).bold().on(Color::DarkGray),
                ),
        );

        // Set up the required keybindings
        let edit_mode = edit_mode(keybindings);

        let editor = editor.or_else(|| resolve_fallback_editor());

        let line_editor = Reedline::create()
            .with_completer(completer)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(edit_mode)
            .with_highlighter(Box::new(Highlighter::default()));

        let line_editor = if let Some(editor) = &editor {
            line_editor.with_buffer_editor(Command::new(editor), tempfile.path_buf().clone())
        } else {
            line_editor
        };

        Repl {
            line_editor,
            prompt,
            tempfile,
            editor,
        }
    }

    pub(crate) fn edit(&mut self, msg_buf: &mut MessageBuffer) -> Option<String> {
        loop {
            let sig = self.line_editor.read_line(&self.prompt);

            match sig {
                Ok(Signal::Success(command)) => {
                    let command_msg = Message::command(command.clone());
                    msg_buf.add_message(command_msg);

                    match command.as_str() {
                        "/exit" => break,
                        "/edit" => {
                            let editor = match self.editor.as_ref() {
                                Some(editor) => editor,
                                None => {
                                    let warning = Message::warn("no editor specified".to_string());
                                    eprintln!("{}", warning);
                                    msg_buf.add_message(warning);
                                    continue;
                                }
                            };

                            let buffer = read_from_interactive_editor(editor, &mut self.tempfile);

                            if buffer.is_empty() {
                                continue;
                            }

                            println!("{}", buffer);

                            return Some(buffer);
                        }
                        "/clear" => {
                            msg_buf.clear();
                            continue;
                        }
                        _ => return Some(command),
                    };
                }
                Ok(Signal::CtrlD) => {
                    break;
                }
                Ok(Signal::CtrlC) => {
                    continue;
                }
                _ => break,
            }
        }

        None
    }
}
