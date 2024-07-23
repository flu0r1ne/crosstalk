use bytes::Bytes;
use futures_core::Stream;
use reqwest::{Client, IntoUrl};
use serde::{Deserialize, Serialize};

use crate::providers::apireq;
use crate::providers::apireq::{JsonStreamParser, ReqwestResponseStreamExt, Url};

#[derive(thiserror::Error, Debug)]
pub(super) enum Error {
    /// The API Base is not a URL that can be used in a network request
    #[error("invalid api base")]
    InvalidApiBase(#[source] reqwest::Error),

    /// Endpoint URL is invalid
    #[error("invalid endpoint")]
    InvalidEndpoint(
        #[from]
        #[source]
        url::ParseError,
    ),

    /// A bad response: the parser failed to parse the
    /// response stream
    #[error("failed to parse streamed response")]
    StreamParser(
        #[from]
        #[source]
        apireq::JsonStreamError,
    ),

    /// Some issue with the request
    #[error("{}", .0)]
    RequestFailed(
        #[from]
        #[source]
        apireq::ReqwestError,
    ),

    /// Your request was malformed or missing some required parameters,
    /// such as a token or an input.
    #[error("{}", .0.message)]
    BadRequest(ApiErrorPayload),

    /// An "Authentication" Error is an umbrella error with three possiblities:
    /// (1) Invalid Authentication
    /// (2) The requesting API key is not correct.
    /// (3) Your account is not part of an organization.
    #[error("{}", .0.message)]
    Authentication(ApiErrorPayload),

    /// You don't have access to the requested resource.
    #[error("{}", .0.message)]
    PermissionDenied(ApiErrorPayload),

    /// Requested resource does not exist.
    #[error("{}", .0.message)]
    NotFound(ApiErrorPayload),

    /// The resource was updated by another request.
    #[error("{}", .0.message)]
    Conflict(ApiErrorPayload),

    /// Unable to process the request despite the format being correct.
    #[error("{}", .0.message)]
    UnprocessableEntity(ApiErrorPayload),

    /// You have hit your assigned rate limit.
    #[error("{}", .0.message)]
    RateLimit(ApiErrorPayload),

    /// OpenAI has an internal issue
    #[error("{}", .0.message)]
    InternalError(ApiErrorPayload),

    /// The engine is currently overloaded, please try again later
    #[error("{}", .0.message)]
    ApiOverloaded(ApiErrorPayload),

    /// Some unknown error was returned by the API
    #[error("{}", .0.message)]
    UnknownStatus(ApiErrorPayload),
}

impl Error {
    fn from_status(status: u16, payload: ApiErrorPayload) -> Error {
        match status {
            400 => Error::BadRequest(payload),
            401 => Error::Authentication(payload),
            403 => Error::PermissionDenied(payload),
            404 => Error::NotFound(payload),
            409 => Error::Conflict(payload),
            422 => Error::UnprocessableEntity(payload),
            429 => Error::RateLimit(payload),
            500 => Error::InternalError(payload),
            503 => Error::ApiOverloaded(payload),
            400..=599 => Error::UnknownStatus(payload),
            _ => unimplemented!("unknown error code for OpenAI API"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub(super) enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct ChatMessage {
    pub content: String,
    pub role: Role,
}

/* Structures to serialize /chat/completions */

#[derive(Serialize, Debug)]
struct ChatCompletionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logit_bias: Option<std::collections::HashMap<String, f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

#[derive(Serialize, Debug)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize, Debug)]
struct ChatCompletionRequest<'o> {
    model: &'o str,
    messages: &'o [ChatMessage],
    #[serde(flatten)]
    options: &'o ChatCompletionOptions,
    stream: bool,
    stream_options: StreamOptions,
}

impl Default for ChatCompletionOptions {
    fn default() -> ChatCompletionOptions {
        ChatCompletionOptions {
            temperature: None,
            top_p: None,
            n: None,
            stop: None,
            max_tokens: None,
            seed: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
        }
    }
}

/* Structures to deseralize /chat/completions */

#[derive(Serialize, Deserialize, Debug)]
pub(super) enum FinishReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "content_filter")]
    ContentFilter,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(super) struct Delta {
    pub role: Option<Role>,
    #[serde(default)]
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(super) struct Choice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct Usage {
    pub completion_tokens: usize,
    pub prompt_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

/* API Errors */

#[derive(Deserialize, Debug)]
pub(super) struct ApiErrorPayload {
    message: String,
    #[serde(rename = "type")]
    typ: String,
}

#[derive(Deserialize, Debug)]
struct ApiErrorResponse {
    error: ApiErrorPayload,
}

pub(super) struct StreamingChatResponse<S>
where
    S: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    stream: JsonStreamParser<S>,
}

impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin> StreamingChatResponse<S> {
    pub(super) async fn next(&mut self) -> Option<Result<ChatCompletionChunk, Error>> {
        let delta = self.stream.parse::<ChatCompletionChunk>().await;

        delta.map(|e| e.map_err(|e| e.into()))
    }
}

const DEFAULT_API_BASE: &'static str = "https://api.openai.com";

pub(super) struct OpenAIApi {
    api_base: Url,
    api_key: String,
}

impl OpenAIApi {
    pub(super) fn new<U: IntoUrl>(api_key: &str, api_base: U) -> Result<OpenAIApi, Error> {
        let api_base = api_base.into_url().map_err(|e| Error::InvalidApiBase(e))?;

        Ok(OpenAIApi {
            api_base,
            api_key: api_key.to_string(),
        })
    }

    pub(super) fn with_api_key(api_key: &str) -> OpenAIApi {
        Self::new(api_key, DEFAULT_API_BASE).unwrap()
    }

    pub(super) async fn streaming_chat_completion(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<StreamingChatResponse<impl Stream<Item = reqwest::Result<bytes::Bytes>>>, Error>
    {
        let url = self.api_base.join("/v1/chat/completions")?;

        let options = ChatCompletionOptions::default();

        let res = Client::new()
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&ChatCompletionRequest {
                model,
                messages,
                options: &options,
                stream: true,
                stream_options: StreamOptions {
                    include_usage: true,
                },
            })
            .send()
            .await
            .map_err(|e| Error::RequestFailed(e.into()))?;

        let status = res.status();

        if status.is_success() {
            let res = res.stream_lsse();

            Ok(StreamingChatResponse { stream: res })
        } else {
            let err: ApiErrorResponse = res
                .json()
                .await
                .expect("failed to deseralize an error message from the OpenAI API");

            Err(Error::from_status(status.as_u16(), err.error))
        }
    }
}

mod tests {
    use super::*;
    use serde_json;

    fn env_api_key() -> String {
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set")
    }

    #[tokio::test]
    async fn test_streaming_chat_completion() {
        let api_key: String = env_api_key();

        let api = OpenAIApi::with_api_key(&api_key);

        let messages = [ChatMessage {
            content: "Hello".to_string(),
            role: Role::User,
        }];

        let mut iterator = api
            .streaming_chat_completion("gpt-4o-mini", &messages)
            .await
            .expect("failed to stream response");

        let mut n_chunks = 0;
        let mut finished = false;

        while let Some(chunk) = iterator.next().await {
            let chunk = chunk.expect("failed to parse delta");

            assert_eq!(chunk.object, "chat.completion.chunk");
            assert!(chunk.id.len() > 0);

            // The first message should contain an a role, no content
            if n_chunks == 0 {
                // Should contain the role
                assert!(chunk.choices.len() == 1);

                let choice = &chunk.choices[0];

                assert_eq!(choice.index, 0);
                assert!(matches!(choice.delta.role, Some(Role::Assistant)));
                assert_eq!(choice.delta.content, "");
            // From then on, we should get content until we finish
            } else if !finished {
                // Should either contain content or
                // should finish with a stop
                assert_eq!(chunk.choices.len(), 1);

                let choice = &chunk.choices[0];

                assert_eq!(choice.index, 0);
                assert!(matches!(choice.delta.role, None));

                let finish_now = matches!(choice.finish_reason, Some(FinishReason::Stop));

                // We should either have content or get a stop
                assert!((choice.delta.content.len() > 0) != finish_now);

                finished = finished | finish_now;

            // The last messages should only contain usage info
            } else {
                assert!(chunk.choices.len() == 0);

                assert!(chunk.usage.is_some());

                let usage = chunk.usage.unwrap();

                assert!(usage.completion_tokens > 0);
                assert!(usage.prompt_tokens > 0);
                assert!(usage.total_tokens > 0);
            }

            n_chunks += 1;
        }

        assert!(n_chunks > 0)
    }

    #[tokio::test]
    async fn test_model_not_found() {
        let api_key: String = env_api_key();

        let api = OpenAIApi::with_api_key(&api_key);

        let messages = [ChatMessage {
            content: "Hello".to_string(),
            role: Role::User,
        }];

        let it = api
            .streaming_chat_completion("__model_does_not_exist__", &messages)
            .await;

        assert!(matches!(it, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn test_invalid_creds() {
        let api = OpenAIApi::with_api_key("not_a_valid_key");

        let messages = [ChatMessage {
            content: "Hello".to_string(),
            role: Role::User,
        }];

        let it = api
            .streaming_chat_completion("__model_does_not_exist__", &messages)
            .await;

        assert!(matches!(it, Err(Error::Authentication(_))));
    }
}
