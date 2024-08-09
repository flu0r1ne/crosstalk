use crate::cli::ColorMode;
use nu_ansi_term::Color;
use std::sync::atomic::{AtomicBool, Ordering};

pub const DEFAULT_EXIT_CODE: i32 = 1;

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

fn use_color() -> ColorMode {
    match unsafe { USE_COLOR.load(Ordering::Relaxed) } {
        true => ColorMode::On,
        false => ColorMode::Off,
    }
}

pub(crate) fn error_internal(text: &str) {
    match use_color() {
        ColorMode::On => {
            let style = Color::Red.bold();
            let text_style = Color::Default.bold();

            eprintln!("{} {}", style.paint("error:"), text_style.paint(text));
        }
        ColorMode::Off => {
            eprintln!("error: {}", text);
        }
    }
}

pub(crate) fn warn_internal(text: &str) {
    match use_color() {
        ColorMode::On => {
            let style = Color::Yellow.bold();
            let text_style = Color::Default.bold();

            eprintln!("{} {}", style.paint("warning:"), text_style.paint(text));
        }
        ColorMode::Off => {
            eprintln!("warning: {}", text);
        }
    }
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
