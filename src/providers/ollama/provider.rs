use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use reqwest::IntoUrl;

use super::api;
use crate::providers::{
    providers::ProviderIdentifier, AsyncMessageIterator, ChatProvider, ContextManagement, Error,
    ErrorKind, FinishReason, Message, MessageDelta, Model, Role, Usage,
};

impl From<api::Role> for Role {
    fn from(value: api::Role) -> Self {
        match value {
            api::Role::User => Role::User,
            api::Role::System => Role::System,
            api::Role::Assistant => Role::Model,
        }
    }
}

impl From<Role> for api::Role {
    fn from(value: Role) -> Self {
        match value {
            Role::User => api::Role::User,
            Role::System => api::Role::System,
            Role::Model => api::Role::Assistant,
            Role::Info => panic!("Role::Info is not an ollama role"),
        }
    }
}

impl From<api::DoneReason> for FinishReason {
    fn from(value: api::DoneReason) -> Self {
        match value {
            api::DoneReason::Length => FinishReason::Length,
            api::DoneReason::Stop => FinishReason::Stop,
            api::DoneReason::None => panic!("DoneReason::None is not a finish reason"),
        }
    }
}

impl From<api::Tag> for Model {
    fn from(value: api::Tag) -> Self {
        Model {
            id: value.name,
            context_length: None,
        }
    }
}

impl From<api::StreamingChatDelta> for MessageDelta {
    fn from(value: api::StreamingChatDelta) -> Self {
        MessageDelta {
            role: value.message.role.into(),
            content: value.message.content,
        }
    }
}

impl From<api::Error> for Error {
    fn from(value: api::Error) -> Self {
        let kind = match &value {
            api::Error::InternalError(_) => Some(ErrorKind::InternalError),
            api::Error::InvalidApiBase(_) | api::Error::InvalidEndpoint(_) => {
                Some(ErrorKind::Connection)
            }
            api::Error::NotFound(_) => Some(ErrorKind::NotFound),
            api::Error::BadRequest(_) => Some(ErrorKind::BadRequest),
            api::Error::RequestFailed(_) | api::Error::StreamParser(_) => None,
            api::Error::UnspecifiedError(_) => Some(ErrorKind::UnspecifiedError),
        };

        match value {
            api::Error::RequestFailed(err) => err.into(),
            api::Error::StreamParser(err) => err.into(),
            value => Error::from_source(kind.unwrap(), Box::new(value)),
        }
    }
}

pub(crate) struct OllamaProvider {
    api: api::OllamaApi,
}

impl OllamaProvider {
    pub(crate) fn with_api_base<U: IntoUrl>(api_base: U) -> Result<OllamaProvider, Error> {
        Ok(OllamaProvider {
            api: api::OllamaApi::with_api_base(api_base)?,
        })
    }

    pub(crate) fn new() -> OllamaProvider {
        OllamaProvider {
            api: api::OllamaApi::new(),
        }
    }
}

pub(crate) struct OllamaCompletionResponse<S>
where
    S: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    inner: api::StreamingChatResponse<S>,
    usage: Option<Usage>,
    finish_reason: Option<FinishReason>,
}

#[async_trait]
impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin + Send> AsyncMessageIterator
    for OllamaCompletionResponse<S>
{
    async fn next(&mut self) -> Option<Result<MessageDelta, Error>> {
        let delta = self.inner.next().await?;

        match delta {
            Ok(msg) => {
                if msg.done {
                    assert!(!matches!(msg.done_reason, api::DoneReason::None));

                    self.finish_reason = Some(msg.done_reason.into());

                    // The "prompt eval count" disappears when cached.
                    // This makes token counting impossible.
                    self.usage = Some(Usage {
                        prompt_tokens: msg.prompt_eval_count,
                        completion_tokens: msg.eval_count,
                    });

                    None
                } else {
                    Some(Ok(MessageDelta {
                        role: msg.message.role.into(),
                        content: msg.message.content,
                    }))
                }
            }
            Err(err) => Some(Err(err.into())),
        }
    }

    fn finish_reason(&self) -> FinishReason {
        self.finish_reason.unwrap()
    }

    fn usage(&self) -> &Usage {
        self.usage.as_ref().unwrap()
    }
}

#[async_trait]
impl ChatProvider for OllamaProvider {
    fn id(&self) -> ProviderIdentifier {
        ProviderIdentifier::Ollama
    }

    fn context_management(&self) -> ContextManagement {
        ContextManagement::Implicit
    }

    async fn default_model(&self) -> Option<Model> {
        None
    }

    async fn models(&self) -> Result<Vec<Model>, Error> {
        let tags = self.api.tags().await?;

        let models: Vec<Model> = tags.into_iter().map(|t| t.into()).collect();

        Ok(models)
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

        let completion = self.api.chat(model, &messages).await?;

        Ok(Box::new(OllamaCompletionResponse {
            inner: completion,
            finish_reason: None,
            usage: None,
        }))
    }
}
