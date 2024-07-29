use super::registry::{Error, ModelResolver, ModelSpec, Registry};
use crate::providers::providers::{OllamaProvider, OpenAIProvider};
use crate::providers::{ChatProvider, ErrorKind};

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
