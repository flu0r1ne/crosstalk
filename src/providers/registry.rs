//! The registry handles provider and model resolution. It is a database populated
//! with the available providers and models. Only models which are enabled are inserted into
//! the registry. When the user chooses a model, it is specified using a "model spec".
//! This registry is responsible for "resolving" the model and provider from this identifier.
//!
//! This consists of two parts, the provider identifier and the model identifer. In BNF:
//! ```
//! <model spec> := <model identifier> | <provider identifier> "/" <model identifier>
//! ```
//!
//! For example, llama3 can be accessed through the ollama provider using the spec
//! "ollama/llama3" since llama3 could be served by multiple providers. If only
//! the model identifier "llama3" is provided, it will be resolved from a provider that
//! offers it.
//!
//! Each provider is assigned a "priority", which is an eight bit unsigned number (e.g., a value between 0
//! and 255), where 0 is the lowest priority (meaning it is a provider of last resort) and 255 is the
//! highest priority. When there are multiple conflicting providers for a model, the highest priority
//! provider is chosen. If two providers provide the same model and are assigned the same priority,
//! resolution is implementation-dependant.

use crate::providers::{
    self,
    providers::{OllamaProvider, OpenAIProvider},
    Model, ChatProvider, ErrorKind, ProviderIdentifier,
};

use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum Error {
    /// No providers serve the model identifier
    #[error("model {0} not found")]
    ModelNotFound(String),

    /// The model spec contains an unknown provider.
    #[error("provider {0} not found")]
    ProviderNotFound(String),
    /// The provider is not in the registry
    #[error("provider {0} not enabled")]
    ProviderNotEnabled(String),
    /// None of the providers in the registry provide a default model
    #[error("there is no default model")]
    NoDefaultModel,
    /// Failed to list the models from one of the providers in the registry
    #[error("failed to obtain models from provider: {0}")]
    ModelListingFailed(
        #[from]
        #[source]
        providers::Error,
    ),
}

pub(crate) struct ModelResolver {
    models: HashMap<String, (ProviderIdentifier, Model)>,
    default_model: Option<(ProviderIdentifier, Model)>,
}

impl ModelResolver {
    fn new() -> ModelResolver {
        ModelResolver {
            models: HashMap::new(),
            default_model: None,
        }
    }
}

impl<'m> ModelResolver {
    fn resolve_ambigious_model(
        &self,
        model: Option<&str>,
    ) -> Result<(ProviderIdentifier, Model), Error> {
        match model {
            Some(spec) => match self.models.get(spec) {
                Some(id) => Ok(id.clone()),
                None => Err(Error::ModelNotFound(spec.to_string())),
            },
            None => match &self.default_model {
                Some(default) => Ok(default.clone()),
                None => Err(Error::NoDefaultModel),
            },
        }
    }
}

pub(crate) struct Registry {
    providers: HashMap<ProviderIdentifier, Box<dyn ChatProvider>>,
    priority: HashMap<ProviderIdentifier, u8>,
    resolver: Option<ModelResolver>,
}

pub(crate) struct ResolvedModelSpec<'m> {
    pub provider: &'m Box<dyn ChatProvider>,
    pub model_id: String,
}

impl Registry {
    pub(crate) fn new() -> Registry {
        Registry {
            providers: HashMap::new(),
            priority: HashMap::new(),
            resolver: None,
        }
    }

    async fn build_resolver(&mut self) -> Result<(), Error> {
        let mut resolver = ModelResolver::new();

        for (id, provider) in &self.providers {
            let models = provider.models().await?;

            for model in models {
                let alt = resolver.models.get_mut(&model.id);

                if let Some((alt_id, alt_model)) = alt {
                    if self.priority[alt_id] >= self.priority[id] {
                        continue;
                    }

                    *alt_id = *id;
                    *alt_model = model;
                } else {
                    resolver.models.insert(model.id.clone(), (*id, model));
                }
            }

            // Update default if nessesary
            if let Some((alt_id, _)) = &resolver.default_model {
                if self.priority[id] >= self.priority[alt_id] {
                    continue;
                }
            }

            if let Some(default) = provider.default_model().await {
                resolver.default_model = Some((*id, default.clone()));
            }
        }

        self.resolver = Some(resolver);

        Ok(())
    }

    fn default_priority(provider_id: ProviderIdentifier) -> u8 {
        match provider_id {
            ProviderIdentifier::Ollama => 20,
            ProviderIdentifier::OpenAI => 10,
        }
    }

    pub(crate) fn add_provider(&mut self, provider: Box<dyn ChatProvider>, priority: Option<u8>) {
        assert!(
            matches!(self.resolver, None),
            "resolution should only occur after providers have been added"
        );

        let id = provider.id();
        let priority = priority.unwrap_or(Self::default_priority(id));

        if let Some(_) = self.providers.insert(id, provider) {
            panic!("attempt to add two identical providers")
        }

        self.priority.insert(id, priority);
    }

    pub(crate) async fn resolve(
        &mut self,
        model_spec: Option<&str>,
    ) -> Result<ResolvedModelSpec, Error> {
        if let Some(spec) = model_spec.as_ref() {
            if spec.contains('/') {
                let (provider, model) = spec.split_once('/').unwrap();

                let id = ProviderIdentifier::from_str(provider)
                    .map_err(|_| Error::ProviderNotFound(provider.to_string()))?;

                return match self.providers.get(&id) {
                    Some(provider) => Ok(ResolvedModelSpec {
                        provider,
                        model_id: model.to_string(),
                    }),
                    None => Err(Error::ProviderNotEnabled(provider.to_string())),
                };
            }
        }

        if self.resolver.is_none() {
            self.build_resolver().await?;
        }

        let resolver = self.resolver.as_ref().unwrap();

        let result = resolver.resolve_ambigious_model(model_spec)?;

        let provider = self.providers.get(&result.0).unwrap();

        return Ok(ResolvedModelSpec {
            provider,
            model_id: result.1.id,
        });
    }
}

async fn ollama_provider() -> Option<Box<OllamaProvider>> {
    let ollama = OllamaProvider::new();

    let models = ollama.models().await;

    if let Err(err) = models {
        if matches!(err.kind(), ErrorKind::Connection | ErrorKind::TimedOut) {
            return None;
        }

        panic!(
            "unexpected response while attempting to probe ollama: {}",
            err
        );
    }

    Some(Box::new(ollama))
}

pub(crate) async fn populated_registry() -> Registry {
    let mut registry = Registry::new();

    if let Some(provider) = ollama_provider().await {
        registry.add_provider(provider, None);
    }

    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        let openai_provider = Box::new(OpenAIProvider::with_api_key(&api_key));

        registry.add_provider(openai_provider, None);
    }

    registry
}
