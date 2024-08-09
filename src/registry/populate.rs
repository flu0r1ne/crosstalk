use std::env::VarError;

use crate::die;

use super::registry::{Error, ModelResolver, ModelSpec, Registry};
use crate::config::{Config, ProviderActivationPolicy};
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
            VarError::NotUnicode(_) => die!("failed to parse {}", OPENAI_ENV_KEY_VAR),
            VarError::NotPresent => None,
        },
    }
}

/// Populate a registry with the available providers
pub(crate) async fn populated_registry(config: &Config) -> Registry {
    let mut registry = Registry::new();

    {
        let ollama = &config.providers.ollama;

        let provider = match ollama.activate {
            ProviderActivationPolicy::Auto | ProviderActivationPolicy::Enabled => {
                if let Some(api_base) = &ollama.api_base {
                    match OllamaProvider::with_api_base(api_base) {
                        Ok(ollama) => Some(ollama),
                        Err(err) => die!("ollama API base failed to parse: {}", err),
                    }
                } else {
                    Some(OllamaProvider::new())
                }
            }
            ProviderActivationPolicy::Disabled => None,
        };

        match (provider, ollama.activate) {
            (Some(provider), ProviderActivationPolicy::Auto)
                if ollama_is_awake(&provider).await =>
            {
                registry.add_provider(
                    Box::new(provider),
                    ollama.priority,
                    ollama.default_model.clone(),
                );
            }
            (Some(provider), ProviderActivationPolicy::Enabled) => {
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

        let activated = match openai.activate {
            ProviderActivationPolicy::Auto => {
                // Activate if API key is present
                api_key
            }
            ProviderActivationPolicy::Enabled => {
                if api_key.is_none() {
                    die!("the \"openai\" provider is activated but the API key is not defined, either add it to the config or define {}", OPENAI_ENV_KEY_VAR);
                }

                api_key
            }
            ProviderActivationPolicy::Disabled => None,
        };

        if let Some(api_key) = activated {
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

    let provider = registry.active_provider(id)?;

    Ok((provider, model))
}
