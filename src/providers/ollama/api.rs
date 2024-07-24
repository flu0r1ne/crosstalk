use bytes::Bytes;
use futures_core::Stream;
use reqwest::{Client, IntoUrl, Response, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::providers::apireq::{
    self, JsonStreamError, JsonStreamParser, ReqwestResponseStreamExt, Url,
};

const OLLAMA_DEFAULT_ENDPOINT: &'static str = "http://localhost:11434";

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error("invalid ollama api base: {0}")]
    InvalidApiBase(reqwest::Error),

    #[error("invalid ollama endpoint: {0}")]
    InvalidEndpoint(#[from] url::ParseError),

    #[error("a request to ollama failed: {0}")]
    RequestFailed(#[from] apireq::ReqwestError),

    #[error("failed to query ollama resource: {0}")]
    NotFound(String),

    #[error("request to the ollama api failed: {0}")]
    BadRequest(String),

    #[error("ollama encountered an internal error: {0}")]
    InternalError(String),

    #[error("the ollama API returned an unspecified error: {0}")]
    UnspecifiedError(String),

    #[error("could not parse streamed response: {0}")]
    StreamParser(#[from] JsonStreamError),
}

/* === IO === */

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(super) enum Role {
    Assistant,
    User,
    System,
}

// Structures to serialize /api/chat
#[derive(Serialize, Debug)]
pub(super) struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Serialize, Debug)]
struct ChatRequest<'m> {
    model: &'m str,
    messages: &'m [ChatMessage],
}

// Structures to deseralize /api/chat
#[derive(Deserialize, Debug)]
pub(super) struct MessageDelta {
    pub role: Role,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub(super) enum DoneReason {
    None,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
}

impl Default for DoneReason {
    fn default() -> Self {
        DoneReason::None
    }
}

#[derive(Deserialize, Debug)]
pub(super) struct StreamingChatDelta {
    pub message: MessageDelta,
    #[serde(default)]
    pub prompt_eval_count: Option<usize>,
    #[serde(default)]
    pub eval_count: Option<usize>,
    #[serde(default)]
    pub done_reason: DoneReason,
    pub done: bool,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum StreamChatChunk {
    Delta(StreamingChatDelta),
    Error(ApiError),
}

// Structures to deseralize /api/tags

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Tag {
    pub name: String,
    pub model: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
    pub details: Details,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Details {
    pub parent_model: String,
    pub format: String,
    pub family: String,
    pub families: Option<Vec<String>>, // Use Option to handle the null value
    pub parameter_size: String,
    pub quantization_level: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TagsList {
    models: Vec<Tag>,
}

// Errors
#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
}

pub(super) struct StreamingChatResponse<S>
where
    S: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    stream: JsonStreamParser<S>,
}

impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin> StreamingChatResponse<S> {
    pub(crate) async fn next(&mut self) -> Option<Result<StreamingChatDelta, Error>> {
        let delta = self.stream.parse::<StreamChatChunk>().await;

        delta.map(|r| {
            r.map_err(|e| Error::StreamParser(e))
                .and_then(|chunk| match chunk {
                    StreamChatChunk::Delta(d) => Ok(d),
                    StreamChatChunk::Error(e) => Err(Error::UnspecifiedError(e.error)),
                })
        })
    }
}

pub(super) struct OllamaApi {
    api_base: Url,
}

impl OllamaApi {
    pub(super) fn with_api_base<U: IntoUrl>(api_base: U) -> Result<OllamaApi, Error> {
        Ok(OllamaApi {
            api_base: api_base.into_url().map_err(|e| Error::InvalidApiBase(e))?,
        })
    }

    pub(super) fn new() -> OllamaApi {
        Self::with_api_base(OLLAMA_DEFAULT_ENDPOINT).unwrap()
    }

    pub(super) async fn maybe_parse_api_error(res: Response) -> Result<Response, Error> {
        let status = res.status();

        if status.is_success() {
            Ok(res)
        } else {
            let err: ApiError = res
                .json()
                .await
                .expect("failed to deseralize an error message from the Ollama API");

            match status {
                StatusCode::NOT_FOUND => Err(Error::NotFound(err.error)),
                code => match code.as_u16() {
                    400..=499 => Err(Error::BadRequest(err.error)),
                    500..=599 => Err(Error::InternalError(err.error)),
                    _ => Err(Error::UnspecifiedError(err.error)),
                },
            }
        }
    }

    pub(super) async fn tags(&self) -> Result<Vec<Tag>, Error> {
        let url = self.api_base.join("/api/tags")?;

        let res = Client::new()
            .get(url)
            .send()
            .await
            .map_err(|e| Error::RequestFailed(e.into()))?;

        let res = Self::maybe_parse_api_error(res).await?;

        let tags: TagsList = res
            .json()
            .await
            .map_err(|e| Error::RequestFailed(e.into()))?;

        Ok(tags.models)
    }

    pub(super) async fn chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<StreamingChatResponse<impl Stream<Item = reqwest::Result<bytes::Bytes>>>, Error>
    {
        let url = self.api_base.join("/api/chat")?;

        let res = Client::new()
            .post(url)
            .json(&ChatRequest { messages, model })
            .send()
            .await
            .map_err(|e| Error::RequestFailed(e.into()))?;

        let res = Self::maybe_parse_api_error(res).await?;

        let stream = res.stream_ndjson();

        Ok(StreamingChatResponse { stream })
    }
}

// Must have gemma:2b
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_models_list() {
        let api = OllamaApi::new();

        let tags = api.tags().await;

        let tags = tags.unwrap();
        assert!(tags.len() > 0);

        let mut found_gemma2b = false;
        for tag in &tags {
            if tag.name != "gemma:2b" {
                continue;
            }

            found_gemma2b = true;

            assert_eq!(tag.model, "gemma:2b");
            assert_eq!(tag.size, 1678456656);
            assert_eq!(
                tag.digest,
                "b50d6c999e592ae4f79acae23b4feaefbdfceaa7cd366df2610e3072c052a160"
            );
            assert_eq!(tag.details.parent_model, "");
            assert_eq!(tag.details.format, "gguf");
            assert_eq!(tag.details.family, "gemma");
            let families = tag.details.families.as_ref().unwrap();
            assert_eq!(families, &["gemma"]);
            assert_eq!(tag.details.parameter_size, "3B");
            assert_eq!(tag.details.quantization_level, "Q4_0");
        }

        assert!(found_gemma2b);
    }

    #[tokio::test]
    async fn test_api_error_deserialization() {
        let api = OllamaApi::new();

        let messages = [ChatMessage {
            role: Role::User,
            content: "Hello!".to_string(),
        }];

        let stream = api.chat("_nonexistent_", &messages).await;

        assert!(stream.is_err());

        if let Err(err) = stream {
            assert!(matches!(err, Error::NotFound(_)));
        }
    }

    #[tokio::test]
    async fn test_gemma_2b() {
        let api = OllamaApi::new();

        let messages = [ChatMessage {
            role: Role::User,
            content: "Hello!".to_string(),
        }];

        let mut res_stream = api.chat("gemma:2b", &messages).await.unwrap();

        let mut first: Option<StreamingChatDelta> = None;
        let mut last: Option<StreamingChatDelta> = None;

        while let Some(s) = res_stream.next().await {
            assert!(!s.is_err());

            let s = s.unwrap();

            if first.is_none() {
                first = Some(s);
            } else {
                last = Some(s);
            }
        }

        // Failed to recieve messages
        assert!(last.is_some());

        let first = first.unwrap();
        let last = last.unwrap();

        // First message has some content
        assert!(matches!(first.message.role, Role::Assistant));
        assert!(!first.message.content.is_empty());
        assert!(!first.done);

        // We reached a stopping point and generated something
        assert!(last.eval_count.unwrap() > 0);
        assert!(matches!(last.done_reason, DoneReason::Stop));
        assert!(last.done);
    }
}
