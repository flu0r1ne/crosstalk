use std::io::{self, IsTerminal};

use crate::RequestedColorMode;

pub(crate) mod chat;
pub(crate) mod list;

#[derive(Clone, Copy, strum_macros::Display)]
pub(crate) enum ColorMode {
    On,
    Off,
}

impl ColorMode {
    /// Returns whether ANSI color should be used
    /// If the user has specified a preference, this is honored. This preference
    /// can be specified through the command line or the "NO_COLOR" environment
    /// variable If the user hasn't stated a preference, color is enabled if the
    /// output is a terminal.
    pub(crate) fn resolve_auto(cm: RequestedColorMode) -> ColorMode {
        match cm {
            RequestedColorMode::Auto => {
                let disable_color =
                    std::env::var_os("NO_COLOR").is_some() || !io::stdout().is_terminal();

                if disable_color {
                    ColorMode::Off
                } else {
                    ColorMode::On
                }
            }
            RequestedColorMode::On => ColorMode::On,
            RequestedColorMode::Off => ColorMode::Off,
        }
    }
}
