use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use toml;

#[derive(Deserialize, Serialize, Default, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RequestedProviderEnabled {
    #[default]
    Auto,
    Yes,
    No,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct Ollama {
    pub enabled: RequestedProviderEnabled,
    pub default_model: Option<String>,
    pub api_base: Option<String>,
    pub priority: Option<u8>,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct OpenAI {
    pub enabled: RequestedProviderEnabled,
    pub default_model: Option<String>,
    pub api_key: Option<String>,
    pub priority: Option<u8>,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct Providers {
    #[serde(default)]
    pub ollama: Ollama,
    #[serde(default)]
    pub openai: OpenAI,
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub(crate) struct Config {
    pub editor: Option<String>,
    #[serde(default)]
    pub providers: Providers,
}

fn get_config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME");

    if let Some(home) = home {
        let home = PathBuf::from(home);

        const USER_PATHS: [&str; 2] = [".config/crosstalk/config.toml", ".crosstalk.toml"];

        for &path in USER_PATHS.iter() {
            let fullpath = home.join(path);

            if fullpath.exists() {
                return Some(fullpath);
            }
        }
    }

    let system_config = PathBuf::from("/etc/crosstalk.toml");

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
        Err(err) => die::die!("failed to parse config: {}", err),
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

            eprintln!(
                "warning: config contains extraneous key \"{}\", ignoring",
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
