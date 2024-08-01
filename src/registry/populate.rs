use std::env::VarError;

use die::die;

use super::registry::{Error, ModelResolver, ModelSpec, Registry};
use crate::config::{Config, RequestedProviderEnabled};
use crate::providers::providers::{OllamaProvider, OpenAIProvider};
use crate::providers::{ChatProvider, ErrorKind};

async fn ollama_is_awake(ollama: &OllamaProvider) -> bool {
    let models = ollama.models().await;

    if let Err(err) = models {
        if matches!(err.kind(), ErrorKind::Connection | ErrorKind::TimedOut) {
            return false;
        }

        panic!(
            "unexpected response while attempting to probe ollama: {}",
            err
        );
    }

    true
}

const OPENAI_ENV_KEY_VAR: &'static str = "OPENAI_API_KEY";

fn openai_api_key() -> Option<String> {
    match std::env::var(OPENAI_ENV_KEY_VAR) {
        Ok(api_key) => Some(api_key),
        Err(err) => match err {
            VarError::NotUnicode(_) => die::die!("error: failed to parse {}", OPENAI_ENV_KEY_VAR),
            VarError::NotPresent => None,
        },
    }
}

/// Populate a registry with the available providers
pub(crate) async fn populated_registry(config: &Config) -> Registry {
    let mut registry = Registry::new();

    {
        let ollama = &config.providers.ollama;

        let provider = match ollama.enabled {
            RequestedProviderEnabled::Auto | RequestedProviderEnabled::Yes => {
                if let Some(api_base) = &ollama.api_base {
                    match OllamaProvider::with_api_base(api_base) {
                        Ok(ollama) => Some(ollama),
                        Err(err) => die::die!("ollama API base failed to parse: {}", err),
                    }
                } else {
                    Some(OllamaProvider::new())
                }
            }
            RequestedProviderEnabled::No => None,
        };

        match (provider, ollama.enabled) {
            (Some(provider), RequestedProviderEnabled::Auto)
                if ollama_is_awake(&provider).await =>
            {
                registry.add_provider(
                    Box::new(provider),
                    ollama.priority,
                    ollama.default_model.clone(),
                );
            }
            (Some(provider), RequestedProviderEnabled::Yes) => {
                registry.add_provider(
                    Box::new(provider),
                    ollama.priority,
                    ollama.default_model.clone(),
                );
            }
            _ => {}
        }
    }

    {
        let openai = &config.providers.openai;
        let openai_env_var = openai_api_key();

        let api_key = if let Some(api_key) = &openai.api_key {
            Some(api_key)
        } else if let Some(api_key) = &openai_env_var {
            Some(api_key)
        } else {
            None
        };

        let enabled = match openai.enabled {
            RequestedProviderEnabled::Auto => {
                // Enable if API key is present
                api_key
            }
            RequestedProviderEnabled::Yes => {
                if api_key.is_none() {
                    die::die!("the \"openai\" provider is enabled but the API key is not defined, either add it to the config or define {}", OPENAI_ENV_KEY_VAR);
                }

                api_key
            }
            RequestedProviderEnabled::No => None,
        };

        if let Some(api_key) = enabled {
            let provider = Box::new(OpenAIProvider::with_api_key(&api_key));

            registry.add_provider(provider, openai.priority, openai.default_model.clone());
        }
    }

    registry
}

/// Resolve a single model
pub(crate) async fn resolve_once<'r>(
    registry: &'r Registry,
    raw_spec: Option<String>,
) -> Result<(&'r Box<dyn ChatProvider>, String), Error> {
    let spec = ModelSpec::parse(raw_spec)?;

    let spec = if spec.is_ambiguous() {
        let resolver = ModelResolver::build(&registry).await?;

        resolver.resolve(spec)?
    } else {
        spec
    };

    let (id, model) = spec.unwrap_provider_model_ids();

    let provider = registry.enabled_provider(id)?;

    Ok((provider, model))
}
