use nu_ansi_term::AnsiGenericString;
use reedline::{
    self, Color, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, PromptViMode,
};
use std::borrow::Cow;

use crate::color::{self, MaybePaint};

const USER_PROMPT: &'static str = "[#] ";
const USER_VI_INSERT_PROMPT: &'static str = USER_PROMPT;
const USER_VI_NORMAL_PROMPT: &'static str = "[=] ";
const COMPLETION_MARKER: &'static str = "[/] ";
const USER_MULTLINE_PROMPT: &'static str = "::: ";

pub(crate) fn model_prompt(model_name: &str) -> String {
    let prompt_text = format!("[{}] ", model_name);

    color::MODEL_PROMPT.maybe_paint(prompt_text).to_string()
}

pub(crate) fn user_prompt() -> AnsiGenericString<'static, str> {
    color::USER_PROMPT.maybe_paint(USER_PROMPT)
}

pub(crate) fn user_vi_insert_prompt() -> AnsiGenericString<'static, str> {
    color::USER_PROMPT.maybe_paint(USER_VI_INSERT_PROMPT)
}

pub(crate) fn user_vi_normal_prompt() -> AnsiGenericString<'static, str> {
    color::USER_PROMPT.maybe_paint(USER_VI_NORMAL_PROMPT)
}

pub(crate) fn completion_marker() -> AnsiGenericString<'static, str> {
    color::USER_PROMPT.maybe_paint(COMPLETION_MARKER)
}

pub(crate) fn multiline_prompt() -> AnsiGenericString<'static, str> {
    color::USER_PROMPT.maybe_paint(USER_MULTLINE_PROMPT)
}

pub(crate) struct Prompt {
    user_prompt: String,
    user_vi_normal_prompt: String,
    user_vi_insert_prompt: String,
    user_multiline_prompt: String,
}

impl Default for Prompt {
    fn default() -> Self {
        Prompt {
            user_prompt: user_prompt().to_string(),
            user_vi_insert_prompt: user_vi_insert_prompt().to_string(),
            user_vi_normal_prompt: user_vi_normal_prompt().to_string(),
            user_multiline_prompt: multiline_prompt().to_string(),
        }
    }
}

impl reedline::Prompt for Prompt {
    fn render_prompt_left(&self) -> std::borrow::Cow<str> {
        Cow::Borrowed("")
    }

    fn render_prompt_right(&self) -> std::borrow::Cow<str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, prompt_mode: reedline::PromptEditMode) -> Cow<str> {
        match prompt_mode {
            PromptEditMode::Default | PromptEditMode::Emacs => Cow::Borrowed(&self.user_prompt),
            PromptEditMode::Vi(vi_mode) => match vi_mode {
                PromptViMode::Normal => Cow::Borrowed(&self.user_vi_normal_prompt),
                PromptViMode::Insert => Cow::Borrowed(&self.user_vi_insert_prompt),
            },
            PromptEditMode::Custom(_) => unimplemented!("custom edit modes are not in use"),
        }
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<str> {
        Cow::Borrowed(&self.user_multiline_prompt)
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        // NOTE: magic strings, given there is logic on how these compose I am not sure if it
        // is worth extracting in to static constant
        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            prefix, history_search.term
        ))
    }
}
