use super::default_priority::default_priority;

use crate::providers::{self, providers::ProviderIdentifier, ChatProvider, Model};
use core::fmt;
use std::collections::HashMap;
use std::default;
use std::str::FromStr;
use strum::IntoEnumIterator;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum Error {
    /// No providers serve the model identifier
    #[error("model \"{0}\" is not served by any of the available providers")]
    ModelNotFound(String),
    /// The model spec contains an unknown provider.
    #[error("provider \"{0}\" does not exist")]
    ProviderNotFound(String),
    /// The provider is not in the registry
    #[error("provider \"{0}\" is not activate")]
    ProviderNotActivated(String),
    /// None of the providers in the registry provide a default model
    #[error("none of the available providers provide a default model")]
    DefaultModelUnset,
    /// Failed to list the models from one of the providers in the registry
    #[error("failed to obtain models from provider \"{0}\": \"{1}\"")]
    ModelListingFailed(ProviderIdentifier, #[source] providers::Error),
    #[error("failed to obtain the default model for provider \"{0}\": \"{1}\"")]
    DefaultModelFailed(ProviderIdentifier, #[source] providers::Error),
}

#[derive(Default)]
pub(crate) struct ModelSpec {
    pub provider: Option<ProviderIdentifier>,
    pub model: Option<String>,
}

impl fmt::Display for ModelSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.model.is_none() {
            return write!(f, "default_model");
        }

        if let Some(provider) = self.provider {
            write!(f, "{}/", provider)?;
        }

        write!(f, "{}", self.model.as_ref().unwrap())
    }
}

impl ModelSpec {
    pub(crate) fn resolved(provider: ProviderIdentifier, model: String) -> ModelSpec {
        ModelSpec {
            provider: Some(provider),
            model: Some(model),
        }
    }

    pub(crate) fn unwrap_provider_model_ids(self) -> (ProviderIdentifier, String) {
        if self.is_ambiguous() {
            panic!("Cannot unwrap an unresolved model spec");
        }

        (self.provider.unwrap(), self.model.unwrap())
    }
}

pub(crate) trait AsModelId {
    fn model_id(&self) -> Option<&str>;
}

impl AsModelId for ModelSpec {
    fn model_id(&self) -> Option<&str> {
        self.model.as_ref().map(|s| s.as_str())
    }
}

impl AsModelId for &str {
    fn model_id(&self) -> Option<&str> {
        Some(self)
    }
}

impl ModelSpec {
    pub(crate) fn parse(spec: Option<String>) -> Result<ModelSpec, Error> {
        match spec {
            Some(spec) => {
                if let Some((provider, model)) = spec.split_once('/') {
                    let id = ProviderIdentifier::from_str(provider)
                        .map_err(|_| Error::ProviderNotFound(provider.to_string()))?;

                    Ok(ModelSpec {
                        provider: Some(id),
                        model: Some(model.to_string()),
                    })
                } else {
                    Ok(ModelSpec {
                        provider: None,
                        model: Some(spec),
                    })
                }
            }
            None => Ok(ModelSpec::default()),
        }
    }

    pub(crate) fn is_ambiguous(&self) -> bool {
        self.provider.is_none() || self.model.is_none()
    }

    pub(crate) fn provider(&self) -> Option<ProviderIdentifier> {
        self.provider
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_ref().map(|s| s.as_str())
    }
}

struct ProviderEntry {
    provider: Option<Box<dyn ChatProvider>>,
    priority: u8,
    default_model: Option<String>,
}

pub(crate) struct Registry {
    providers: HashMap<ProviderIdentifier, ProviderEntry>,
}

pub(crate) struct ProvidedModel {
    pub provider: ProviderIdentifier,
    pub model: Model,
}

pub(crate) struct ProvidedDefaultModel {
    pub provider: ProviderIdentifier,
    pub default_model_id: Option<String>,
}

impl From<ProvidedModel> for ModelSpec {
    fn from(value: ProvidedModel) -> Self {
        ModelSpec {
            provider: Some(value.provider),
            model: Some(value.model.id),
        }
    }
}

impl Registry {
    pub(crate) fn new() -> Registry {
        let providers = ProviderIdentifier::iter().map(|id| {
            (
                id,
                ProviderEntry {
                    provider: None,
                    priority: default_priority(id),
                    default_model: None,
                },
            )
        });

        Registry {
            providers: HashMap::from_iter(providers),
        }
    }

    pub(crate) fn add_provider(
        &mut self,
        provider: Box<dyn ChatProvider>,
        priority: Option<u8>,
        default_model: Option<String>,
    ) {
        let id = provider.id();

        let entry = self.providers.get_mut(&id).unwrap();

        if entry.provider.is_some() {
            panic!("The same provider was added to the registry twice.");
        }

        entry.provider.replace(provider);

        if let Some(priority) = priority {
            entry.priority = priority;
        }

        entry.default_model = default_model;
    }

    pub(crate) fn empty(&self) -> bool {
        for (_, ent) in self.providers.iter() {
            if ent.provider.is_some() {
                return false;
            }
        }

        true
    }

    pub(crate) fn provider(&self, id: ProviderIdentifier) -> Option<&Box<dyn ChatProvider>> {
        let ent = self.providers.get(&id).unwrap();

        ent.provider.as_ref()
    }

    pub(crate) fn active_provider(
        &self,
        id: ProviderIdentifier,
    ) -> Result<&Box<dyn ChatProvider>, Error> {
        match self.provider(id) {
            Some(provider) => Ok(provider),
            None => Err(Error::ProviderNotActivated(id.to_string())),
        }
    }

    pub(crate) fn priority(&self, id: ProviderIdentifier) -> u8 {
        let ent = self.providers.get(&id).unwrap();

        ent.priority
    }

    pub(crate) async fn registred_models(&self) -> Result<Vec<ProvidedModel>, Error> {
        let mut models = Vec::new();

        for id in ProviderIdentifier::iter() {
            let provider = match self.provider(id) {
                Some(provider) => provider,
                None => continue,
            };

            let provider_models = provider
                .models()
                .await
                .map_err(|e| Error::ModelListingFailed(id, e))?;

            for model in provider_models {
                models.push(ProvidedModel {
                    provider: id,
                    model: model,
                });
            }
        }

        Ok(models)
    }

    pub(crate) async fn default_models(&self) -> Result<Vec<ProvidedDefaultModel>, Error> {
        let mut models = Vec::new();

        for id in ProviderIdentifier::iter() {
            let ProviderEntry {
                provider,
                priority: _,
                default_model,
            } = self.providers.get(&id).unwrap();

            let provider = match provider {
                Some(provider) => provider,
                None => continue,
            };

            let default_model = if default_model.is_none() {
                provider
                    .default_model()
                    .await
                    .map_err(|e| Error::DefaultModelFailed(id, e))?
                    .map(|model| model.id)
            } else {
                default_model.clone()
            };

            models.push(ProvidedDefaultModel {
                provider: id,
                default_model_id: default_model,
            });
        }

        Ok(models)
    }
}

pub(crate) struct ModelResolver {
    models: HashMap<String, ProviderIdentifier>,
    default_model: Option<(String, ProviderIdentifier)>,
}

impl ModelResolver {
    pub(crate) async fn build(registry: &Registry) -> Result<ModelResolver, Error> {
        let mut resolver = ModelResolver {
            models: HashMap::new(),
            default_model: None,
        };

        for ProvidedModel {
            provider: id,
            model,
        } in registry.registred_models().await?
        {
            if let Some(alt_id) = resolver.models.get_mut(&model.id) {
                if registry.priority(*alt_id) >= registry.priority(id) {
                    continue;
                }

                *alt_id = id;
            } else {
                resolver.models.insert(model.id, id);
            }
        }

        for ProvidedDefaultModel {
            provider: id,
            default_model_id,
        } in registry.default_models().await?
        {
            let default = match default_model_id {
                Some(default) => default,
                None => continue,
            };

            if let Some((_, alt_id)) = resolver.default_model.as_ref() {
                if registry.priority(*alt_id) >= registry.priority(id) {
                    continue;
                }
            }

            resolver.default_model = Some((default, id));
        }

        Ok(resolver)
    }

    pub(crate) fn resolve<S: AsModelId>(&self, spec: S) -> Result<ModelSpec, Error> {
        match spec.model_id() {
            Some(model_id) => match self.models.get(model_id) {
                Some(id) => Ok(ModelSpec::resolved(*id, model_id.to_string())),
                None => Err(Error::ModelNotFound(model_id.to_string())),
            },
            None => match &self.default_model {
                Some((model_id, id)) => Ok(ModelSpec::resolved(*id, model_id.clone())),
                None => Err(Error::DefaultModelUnset),
            },
        }
    }
}
