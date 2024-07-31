//! The registry handles provider and model resolution. It is a database populated
//! with the available providers. Providers that are enabled are inserted into
//! the registry. The registry provides methods for querying all available providers.
//!
//! One important aspect of the registry is building a model resolver. When the user
//! chooses a model, it is specified using a "model spec." This model resolver is
//! responsible for "resolving" the model and provider from this identifier. Sometimes
//! this is trivial because the user specifies both the model identifier and provider
//! identifier in the model spec. In other cases, the resolver must decide which provider
//! to use.
//!
//! The model spec consists of two parts: the provider identifier and the model identifier. In BNF:
//! ```
//! <model spec> := <model identifier> | <provider identifier> "/" <model identifier>
//! ```
//!
//! For example, llama3 can be accessed through the ollama provider using the spec
//! "ollama/llama3," since llama3 could be served by multiple providers. If only
//! the model identifier "llama3" is provided, it will be resolved from a provider that
//! offers it.
//!
//! Each provider is assigned a "priority," which is an eight-bit unsigned number (e.g., a value between 0
//! and 255), where 0 is the lowest priority (meaning it is a provider of last resort) and 255 is the
//! highest priority. When there are multiple conflicting providers for a model, the highest priority
//! provider is chosen. If two providers offer the same model and are assigned the same priority,
//! resolution is implementation-dependent.
//!
//! To see how model resolution works, see [`populate::resolve_once`].

pub(crate) mod populate;
pub(crate) mod registry;

use registry::{ModelResolver, ModelSpec, ProvidedDefaultModel, ProvidedModel, Registry};

mod default_priority;
