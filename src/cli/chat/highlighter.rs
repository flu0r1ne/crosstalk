use crate::color;

#[derive(Default)]
pub(crate) struct Highlighter;

impl reedline::Highlighter for Highlighter {
    fn highlight(&self, line: &str, cursor: usize) -> reedline::StyledText {
        reedline::StyledText {
            buffer: vec![(color::USER_TEXT.clone(), line.to_string())],
        }
    }
}
