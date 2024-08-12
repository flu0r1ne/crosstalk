use crate::color::{self, MaybePaint};

pub const DEFAULT_EXIT_CODE: i32 = 1;

pub(crate) fn fmt_error<S: AsRef<str>>(f: &mut std::fmt::Formatter, text: S) -> std::fmt::Result {
    let text: &str = text.as_ref();

    write!(
        f,
        "{} {}",
        color::ERROR_INDICATOR.maybe_paint("error:"),
        color::WARNING_TEXT.maybe_paint(text),
    )
}

pub(crate) fn fmt_warn<S: AsRef<str>>(f: &mut std::fmt::Formatter, text: &str) -> std::fmt::Result {
    let text: &str = text.as_ref();

    write!(
        f,
        "{} {}",
        color::WARNING_INDICATOR.maybe_paint("warning:"),
        color::WARNING_TEXT.maybe_paint(text),
    )
}

pub(crate) fn error_internal(text: &str) {
    eprintln!(
        "{} {}",
        color::ERROR_INDICATOR.maybe_paint("error:"),
        color::WARNING_TEXT.maybe_paint(text),
    );
}

pub(crate) fn warn_internal(text: &str) {
    eprintln!(
        "{} {}",
        color::WARNING_INDICATOR.maybe_paint("warning:"),
        color::WARNING_TEXT.maybe_paint(text),
    );
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => ({
        let formatted = format!($($arg)*);
        $crate::utils::errors:: warn_internal(&formatted);
    })
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => ({
        let formatted = format!($($arg)*);
        $crate::utils::errors:: error_internal(&formatted);
    })
}

#[macro_export]
macro_rules! die {
    ($($arg:tt)*) => ({
        let formatted = format!($($arg)*);
        $crate::utils::errors::error_internal(&formatted);
        ::std::process::exit($crate::utils::errors::DEFAULT_EXIT_CODE);
    })
}
