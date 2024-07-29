//! Concrete types for providers, along with their provider alias variants

use strum_macros;

/// The `ProviderIdentifier` is a unique per-provider identifier. It is used to
/// differentiate providers at runtime in code which is generic over different
/// providers.
///
/// The `to_string` and `FromStr` are part of the CLI and should remain stable.
#[derive(
    Debug,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum ProviderIdentifier {
    Ollama,
    OpenAI,
}

pub(crate) use super::ollama::OllamaProvider;
pub(crate) use super::openai::OpenAIProvider;
