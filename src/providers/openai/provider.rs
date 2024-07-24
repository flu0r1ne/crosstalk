use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use reqwest::IntoUrl;

use crate::chat::{Message, Role};
use crate::providers::openai::models::{DEFAULT_MODEL, OPENAI_MODELS};
use crate::providers::{
    openai::api, providers::ProviderIdentifier, ChatProvider, Error, ErrorKind, Model,
};
use crate::providers::{
    AsyncMessageIterator, ContextManagement, FinishReason, MessageDelta, Usage,
};

impl From<api::Error> for Error {
    fn from(value: api::Error) -> Self {
        let kind = match &value {
            api::Error::Authentication(_) | api::Error::PermissionDenied(_) => {
                Some(ErrorKind::Authentication)
            }
            api::Error::BadRequest(_)
            | api::Error::InvalidApiBase(_)
            | api::Error::InvalidEndpoint(_)
            | api::Error::UnprocessableEntity(_) => Some(ErrorKind::BadRequest),
            // Request invalidated by a race condition
            api::Error::Conflict(_) => Some(ErrorKind::BadRequest),
            api::Error::InternalError(_) => Some(ErrorKind::InternalError),
            api::Error::NotFound(_) => Some(ErrorKind::NotFound),
            api::Error::RateLimit(_) => Some(ErrorKind::ExcessUsage),
            api::Error::UnknownStatus(_) => Some(ErrorKind::UnspecifiedError),
            api::Error::ApiOverloaded(_) => Some(ErrorKind::ApiOverloaded),

            api::Error::RequestFailed(_) => None,
            api::Error::StreamParser(_) => None,
        };

        match value {
            api::Error::RequestFailed(err) => err.into(),
            api::Error::StreamParser(err) => err.into(),
            value => Error::from_source(kind.unwrap(), Box::new(value)),
        }
    }
}

pub(crate) struct OpenAIProvider {
    api: api::OpenAIApi,
}

impl OpenAIProvider {
    pub(crate) fn new<U: IntoUrl>(api_key: &str, api_base: U) -> Result<OpenAIProvider, Error> {
        Ok(OpenAIProvider {
            api: api::OpenAIApi::new(api_key, api_base)?,
        })
    }

    pub(crate) fn with_api_key(api_key: &str) -> OpenAIProvider {
        OpenAIProvider {
            api: api::OpenAIApi::with_api_key(api_key),
        }
    }
}

impl From<api::FinishReason> for FinishReason {
    fn from(value: api::FinishReason) -> Self {
        match value {
            api::FinishReason::Stop => FinishReason::Stop,
            api::FinishReason::ContentFilter => FinishReason::ContentFilter,
            api::FinishReason::Length => FinishReason::Length,
        }
    }
}

impl From<api::Role> for Role {
    fn from(value: api::Role) -> Self {
        match value {
            api::Role::Assistant => Role::Model,
            api::Role::System => Role::System,
            api::Role::User => Role::User,
            api::Role::Tool => unimplemented!("The provider API does not support tool calls."),
        }
    }
}

pub(crate) struct OpenAICompletionResponse<S>
where
    S: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    inner: api::StreamingChatResponse<S>,
    role: Option<Role>,
    finish_reason: Option<FinishReason>,
    usage: Option<Usage>,
}

impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin + Send> OpenAICompletionResponse<S> {
    fn new(inner: api::StreamingChatResponse<S>) -> OpenAICompletionResponse<S> {
        OpenAICompletionResponse {
            inner,
            role: None,
            finish_reason: None,
            usage: None,
        }
    }
}

impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin + Send> From<api::StreamingChatResponse<S>>
    for OpenAICompletionResponse<S>
{
    fn from(value: api::StreamingChatResponse<S>) -> Self {
        OpenAICompletionResponse::new(value)
    }
}

#[async_trait]
impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin + Send> AsyncMessageIterator
    for OpenAICompletionResponse<S>
{
    async fn next(&mut self) -> Option<Result<MessageDelta, Error>> {
        loop {
            let result = match self.inner.next().await? {
                Ok(mut chunk) => {
                    if chunk.usage.is_some() {
                        debug_assert_eq!(chunk.choices.len(), 0);

                        let usage = chunk.usage.unwrap();

                        self.usage = Some(Usage {
                            prompt_tokens: Some(usage.prompt_tokens),
                            completion_tokens: Some(usage.completion_tokens),
                        });

                        None
                    } else {
                        debug_assert_eq!(chunk.choices.len(), 1);

                        let choice = std::mem::take(&mut chunk.choices[0]);

                        // Skip this chunk, return finish reason with the metadata chunk
                        if let Some(finish_reason) = choice.finish_reason {
                            self.finish_reason = Some(finish_reason.into());
                            continue;
                        }

                        if let Some(role) = choice.delta.role {
                            self.role = Some(role.into());
                        }

                        Some(Ok(MessageDelta {
                            role: self.role.clone().unwrap(),
                            content: choice.delta.content,
                        }))
                    }
                }
                Err(err) => Some(Err(err.into())),
            };

            return result;
        }
    }

    fn finish_reason(&self) -> FinishReason {
        self.finish_reason.unwrap()
    }

    fn usage(&self) -> &Usage {
        self.usage.as_ref().unwrap()
    }
}

impl From<Role> for api::Role {
    fn from(value: Role) -> Self {
        match value {
            Role::Info => unimplemented!("info messages have no API corollary"),
            Role::Model => api::Role::Assistant,
            Role::System => api::Role::System,
            Role::User => api::Role::User,
        }
    }
}

#[async_trait]
impl ChatProvider for OpenAIProvider {
    fn id(&self) -> ProviderIdentifier {
        ProviderIdentifier::OpenAI
    }

    fn context_management(&self) -> ContextManagement {
        ContextManagement::Explicit
    }

    async fn default_model(&self) -> Option<Model> {
        Some(DEFAULT_MODEL.clone())
    }

    async fn models(&self) -> Result<Vec<Model>, Error> {
        Ok(OPENAI_MODELS.to_vec())
    }

    async fn stream_completion(
        &self,
        model: &str,
        messages: &[Message],
    ) -> Result<Box<dyn AsyncMessageIterator>, Error> {
        let messages: Vec<api::ChatMessage> = messages
            .iter()
            .map(|m| api::ChatMessage {
                role: m.role.clone().into(),
                content: m.content.clone(),
            })
            .collect();

        let iterator = self.api.streaming_chat_completion(model, &messages).await?;

        Ok(Box::new(OpenAICompletionResponse::new(iterator)))
    }
}
