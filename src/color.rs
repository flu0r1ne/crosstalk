use crate::cli::ColorMode;
use lazy_static::lazy_static;
use nu_ansi_term::{AnsiGenericString, Color, Style};
use std::borrow::Cow;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

lazy_static! {
    pub(crate) static ref USER_PROMPT: Style = Color::Blue.bold();
    pub(crate) static ref MODEL_PROMPT: Style = Color::Green.bold();
    pub(crate) static ref USER_TEXT: Style = Color::Default.bold();
    pub(crate) static ref MODEL_TEXT: Style = Color::Default.normal();
    pub(crate) static ref ERROR_INDICATOR: Style = Color::Red.bold();
    pub(crate) static ref WARNING_INDICATOR: Style = Color::Yellow.bold();
    pub(crate) static ref ERROR_TEXT: Style = Color::Default.bold();
    pub(crate) static ref WARNING_TEXT: Style = Color::Default.bold();
}

static mut USE_COLOR: AtomicBool = AtomicBool::new(true);

pub(crate) fn configure_color(cmode: ColorMode) {
    match cmode {
        ColorMode::On => unsafe {
            USE_COLOR.store(true, Ordering::Relaxed);
        },
        ColorMode::Off => unsafe {
            USE_COLOR.store(false, Ordering::Relaxed);
        },
    }
}

pub(crate) fn color_mode() -> ColorMode {
    match unsafe { USE_COLOR.load(Ordering::Relaxed) } {
        true => ColorMode::On,
        false => ColorMode::Off,
    }
}

pub(crate) trait MaybePaint {
    #[must_use]
    fn maybe_paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Cow<'a, S>>,
        <S as ToOwned>::Owned: fmt::Debug;
}

impl MaybePaint for Style {
    fn maybe_paint<'a, I, S: 'a + ToOwned + ?Sized>(self, input: I) -> AnsiGenericString<'a, S>
    where
        I: Into<Cow<'a, S>>,
        <S as ToOwned>::Owned: fmt::Debug,
    {
        match color_mode() {
            ColorMode::On => self.paint(input),
            ColorMode::Off => {
                let cow: Cow<'a, S> = input.into();

                cow.into()
            }
        }
    }
}
