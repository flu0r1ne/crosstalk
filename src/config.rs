use crate::die;
use crate::warn;
use serde::{Deserialize, Serialize};
use std::default;
use std::path::PathBuf;
use toml;

/// Specifies when the provider should activate.
#[derive(Deserialize, Serialize, Default, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ProviderActivationPolicy {
    /// Automatically determine if the provider should be considered active (default).
    #[default]
    Auto,
    /// Enforce activation of the provider, returning an error if the activation criteria cannot be met.
    Enabled,
    /// Enforce deactivation of the provider.
    Disabled,
}

/// Specifies the keybindings to be used in the chat REPL.
#[derive(Deserialize, Serialize, Default, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Keybindings {
    /// Use Emacs-style keybindings (default).
    #[default]
    Emacs,
    /// Use Vi-style keybindings.
    Vi,
}

/// Configuration for the Ollama provider.
#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct Ollama {
    /// The activation policy for Ollama.
    ///
    /// If Ollama is always available, setting this to "enabled"
    /// will eliminate a redundant API call to the Ollama server made
    /// at system startup.
    #[serde(default)]
    pub activate: ProviderActivationPolicy,

    /// Specifies the default model to be used when Ollama is the preferred provider.
    pub default_model: Option<String>,

    /// Specifies the base URL for the Ollama API.
    pub api_base: Option<String>,

    /// Sets the priority for the Ollama provider.
    pub priority: Option<u8>,
}

/// Configuration for the OpenAI provider.
#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct OpenAI {
    /// The activation policy for OpenAI.
    #[serde(default)]
    pub activate: ProviderActivationPolicy,

    /// Specifies the default model to be used when OpenAI is the preferred provider.
    pub default_model: Option<String>,

    /// Sets the OpenAI API key. This takes precedence over the OPENAI_API_KEY environment variable, if set.
    pub api_key: Option<String>,

    /// Sets the priority for the OpenAI provider.
    pub priority: Option<u8>,
}

/// Configuration for the providers.
#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct Providers {
    /// Configuration for the Ollama provider.
    #[serde(default)]
    pub ollama: Ollama,

    /// Configuration for the OpenAI provider.
    #[serde(default)]
    pub openai: OpenAI,
}

/// Main configuration structure.
#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct Config {
    /// Specifies the command used to launch an external editor.
    ///
    /// This should specify a binary to be used as the external editor. It
    /// can either be an absolute path to a binary or a command in the PATH
    /// environment variable. It should accept a file as the first argument.
    /// If the editor exits with a zero status, the content in the file will
    /// be used for a prompt.
    pub editor: Option<String>,

    /// Specifies the default model.
    ///
    /// This sets the default chat model and overrides defaults specified by
    /// other providers. It should be set in the form of a model spec.
    pub default_model: Option<String>,

    /// Specifies the keybindings to be used within the chat REPL.
    ///
    /// Acceptable values are "vi" or "emacs". By default, Emacs-style
    /// bindings are used.
    #[serde(default)]
    pub keybindings: Keybindings,

    /// Configuration for the providers.
    #[serde(default)]
    pub providers: Providers,
}

fn get_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME");

    if let Some(home) = home {
        let home = PathBuf::from(home);

        const USER_PATHS: [&str; 2] = [".config/xtalk/config.toml", ".xtalk.toml"];

        for &path in USER_PATHS.iter() {
            let fullpath = home.join(path);

            if fullpath.exists() {
                return Some(fullpath);
            }
        }
    }

    let system_config = PathBuf::from("/etc/xtalk.toml");

    if system_config.exists() {
        Some(system_config)
    } else {
        None
    }
}

fn parse_config_or_die<'de, S: serde::de::DeserializeOwned>(config: &str) -> S {
    let r: Result<S, toml::de::Error> = toml::de::from_str(config);

    match r {
        Ok(s) => s,
        Err(err) => die!("failed to parse config: {}", err),
    }
}

fn warn_on_extra_fields_helper<'a>(
    path: &mut Vec<&'a String>,
    user_config: &'a toml::Table,
    config: &'a toml::Table,
) {
    for (user_key, user_value) in user_config {
        path.push(user_key);

        if let Some(config_value) = config.get(user_key) {
            assert!(
                user_value.same_type(config_value),
                "user value doesn't match config value"
            );

            match (user_value, config_value) {
                (toml::Value::Table(user_value), toml::Value::Table(config_value)) => {
                    warn_on_extra_fields_helper(path, user_value, config_value)
                }
                _ => {}
            }
        } else {
            let path: Vec<&str> = path.iter().map(|&s| s.as_str()).collect();

            warn!(
                "config contains extraneous key \"{}\", ignoring",
                path.join(".")
            );
        }

        path.pop();
    }
}

fn warn_on_extra_fields(config: &Config, raw_config: &str) {
    let user_config: toml::Table = parse_config_or_die(raw_config);

    let config: toml::Table = {
        let seralized_config = toml::ser::to_string(&config).expect("failed to reserialize config");

        parse_config_or_die(&seralized_config)
    };

    let mut path = Vec::new();

    warn_on_extra_fields_helper(&mut path, &user_config, &config);
}

pub(crate) fn read_config(config: Option<PathBuf>) -> Config {
    let config_path = config.or_else(get_config_path);

    if let Some(path) = config_path {
        let raw_config = std::fs::read_to_string(path).expect("failed to read config");

        let config: Config = parse_config_or_die(&raw_config);

        warn_on_extra_fields(&config, &raw_config);

        config
    } else {
        Config::default()
    }
}
