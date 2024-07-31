//! TODO: This needs to be updated.
//!
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

pub(crate) mod populate;
pub(crate) mod registry;

use registry::{ModelResolver, ModelSpec, ProvidedDefaultModel, ProvidedModel, Registry};

mod default_priority;
