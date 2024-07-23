//! Traits and type definitions for chat model completions and provider interactions.
//!
//! The `provider` module contains various components for interacting with chat models.
//! The interface for all conversations is provided by the [`ChatProvider`] trait,
//! which is a general interface for listing models supported by a provider and
//! generating chat completions.
//!
//! ## Chat Providers
//!
//! Each API provider (e.g., OpenAI or Ollama) must implement the [`ChatProvider`] trait to
//! be compatible with crosstalk. Chat providers must support two essential operations:
//! - Models: The models operation should list all the models supported by the completion API.
//! - Completion: The completion operation takes a list of messages and returns a new, model-generated
//!   message.
//!
//! In addition, the [`ChatProvider`] interface provides three additional methods:
//! - Provider Alias: Provides the identity of the provider.
//! - Context Management: Instructs high-level interfaces on how to manage context (e.g., limitations
//!   on the number of chat messages or token context).
//! - Specifies a Default Model: Optionally specifies a default model. If this chat provider is selected
//!   and the user has not specified a model, it will default to this model.
//!
//! ## Error Handling
//!
//! Each API has its own bespoke error systems with varying levels of rigor. For example, the Ollama
//! API documentation does not describe any errors that can be raised by the API, while the OpenAI API
//! is very explicit. In general, providers each have their own error types. These are encapsulated in [`Error`],
//! and the [`ErrorKind`] enum provides an indication of the category of error that was raised.

mod apireq;
mod ollama;
mod openai;

pub(crate) mod providers;
pub(crate) mod registry;

use async_trait::async_trait;
use std::error::Error as StdError;
use std::fmt;

use self::providers::ProviderIdentifier;
use crate::chat::{Message, Role};

/// This is a list specifying general categories of errors that
/// can be returned by a [`ChatProvider`]. This list may be updated
/// as providers are added.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorKind {
    /// Failed to connect to the underlying API service.
    /// This could be due to network issues like DNS
    /// resolution, connectivity issues, or routing problems.
    Connection,
    /// A request timed out.
    TimedOut,
    /// An API key was not provided or service-specific
    /// permissions are needed.
    Authentication,
    /// A rate limit was reached or a quota was exceeded.
    ExcessUsage,
    /// The servers are overloaded. This is non-fatal
    /// and indicates that a retry may be needed later.
    ApiOverloaded,
    /// The requested resource was not found. This likely means that
    /// the model requested by the user was not found.
    NotFound,
    /// The request was malformed or is otherwise improper. This
    /// often corresponds to errors with HTTP status codes in
    /// the 400s.
    BadRequest,
    /// The server encountered an error. This often corresponds to
    /// errors with HTTP status codes in the 500s.
    InternalError,
    /// An API response was unable to be deserialized, malformed,
    /// or otherwise violated the assumptions of the client.
    UnexpectedResponse,
    /// The number of tokens in the request exceeds the maximum limit
    /// imposed on the model.
    ContextExceeded,
    /// An error that does not fit into any of the other categories.
    UnspecifiedError,
}

#[derive(Debug)]
pub(crate) struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl Error {
    pub(crate) fn from_kind(kind: ErrorKind) -> Error {
        Error { kind, source: None }
    }

    pub(crate) fn from_source(kind: ErrorKind, source: Box<dyn StdError + Send + Sync>) -> Error {
        Error {
            kind,
            source: Some(source),
        }
    }

    pub(crate) fn kind(&self) -> ErrorKind {
        self.kind
    }

    fn message(&self) -> &'static str {
        match self.kind {
            ErrorKind::Connection => "failed to connect to the API service",
            ErrorKind::TimedOut => "request timed out",
            ErrorKind::Authentication => "authentication failed or not provided",
            ErrorKind::ExcessUsage => "rate limit exceeded or quota crossed",
            ErrorKind::ApiOverloaded => "API server(s) are currently overloaded",
            ErrorKind::NotFound => "the requested resource was not found",
            ErrorKind::BadRequest => "the request was bad or malformed",
            ErrorKind::InternalError => "the server encountered an internal error",
            ErrorKind::UnexpectedResponse => "API response was unexpected or malformed",
            ErrorKind::UnspecifiedError => "an unspecified error occurred",
            ErrorKind::ContextExceeded => "the model context was exceeded",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|e| &**e as _)
    }
}

/// The reason why the model stopped generating.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FinishReason {
    /// The model generated a stop token, terminating
    /// its response.
    Stop,
    /// An API content filter was triggered.
    ContentFilter,
    /// The requested message length was reached.
    Length,
}

/// A message delta represents a "chunk" of a streamed message.
/// Usually, this consists of a single token.
#[derive(Debug, Clone)]
pub(crate) struct MessageDelta {
    /// The role of the message.
    pub role: Role,
    /// The content of the message.
    pub content: String,
}

/// The context usage metadata.
#[derive(Debug, Clone, Default)]
pub(crate) struct Usage {
    /// The number of tokens in the prompt.
    prompt_tokens: Option<usize>,
    /// The number of tokens in the response.
    completion_tokens: Option<usize>,
}

/// A streamed response from a completion.
#[async_trait]
pub(crate) trait AsyncMessageIterator {
    /// The next chunk of the message.
    async fn next(&mut self) -> Option<Result<MessageDelta, Error>>;

    /// The reason the model stopped generating. This can only be
    /// called once the iterator is exhausted.
    fn finish_reason(&self) -> FinishReason;

    /// The usage for this request. This can only be called once the
    /// iterator is exhausted.
    fn usage(&self) -> &Usage;
}

#[derive(Debug, Clone)]
pub(crate) struct Model {
    /// The ID of the model. This must be an acceptable parameter to
    /// [`ChatProvider::stream_completion`].
    pub id: String,
    /// The context length of the model, if known.
    pub context_length: Option<u64>,
}

/// Provides instructions on how the context should be managed between API
/// calls.
#[derive(Debug, Clone)]
pub(crate) enum ContextManagement {
    /// Implicit management implies that the API automatically manages
    /// the information available to the model. All messages in the conversation
    /// should be fed to the model, and there are no guarantees regarding what messages
    /// are included in the completion.
    Implicit,
    /// The API user must manage the context explicitly. If the token context is exceeded,
    /// the API returns an error of type [`ErrorKind::ContextExceeded`].
    Explicit,
}

/// A trait implemented by all chat providers.
#[async_trait]
pub(crate) trait ChatProvider {
    /// Returns the provider identifier.
    fn id(&self) -> ProviderIdentifier;

    /// Returns the context management strategy.
    fn context_management(&self) -> ContextManagement;

    /// Returns a list of models the chat provider supports.
    async fn models(&self) -> Result<Vec<Model>, Error>;

    /// Returns the default model, or None if no default is designated.
    async fn default_model(&self) -> Option<Model>;

    /// Takes a series of messages that are part of a chat conversation
    /// and produces a new message generated by the model in response.
    ///
    /// `model`: The id of the model.
    /// `messages`: A series of messages in the conversation.
    async fn stream_completion(
        &self,
        model: &str,
        messages: &[Message],
    ) -> Result<Box<dyn AsyncMessageIterator>, Error>;
}
