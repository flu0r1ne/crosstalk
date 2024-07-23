//! This module parses streams of JSON objects from an HTTP response. It supports two
//! formats, newline-delimited JSON and a subset of server-side events. It expects a
//! byte stream, as produced by the [`reqwest::Response::bytes_stream`] method. This
//! can be incrementally parsed, object by object.

use bytes::Bytes;
use core::fmt;
use futures_core::stream::Stream;
use futures_util::StreamExt;
use serde::Deserialize;
use std::error::Error as StdError;
use std::marker::Unpin;

use super::ReqwestError;

trait RemoveFirstN {
    fn remove_first(&mut self, n: usize);
}

impl<T: std::marker::Copy> RemoveFirstN for Vec<T> {
    fn remove_first(&mut self, n: usize) {
        self.copy_within(n.., 0);
        self.truncate(self.len() - n);
    }
}

#[derive(Debug)]
pub(crate) enum StreamFormat {
    /// Newline-delimited Json
    /// See https://github.com/ndjson/ndjson-spec
    Ndjson,
    /// Limited server-side events
    LSSE,
}

#[derive(Debug)]
pub(crate) struct DeseralizationFailedError {
    blob: String,
    error: serde_json::error::Error,
}

// "The Server-Sent-Events parser embedded in crosstalk
// is not spec-compliant. As of 2024 the OpenAI
// only uses it to stream the data buffer so this is all we
// support. If this is changed at some future time, this will
// have to be updated."
#[derive(Debug)]
pub(crate) enum Error {
    // stream is not supported by the parser
    UnsupportedSseFieldName,
    ResponseExceededBuffer,
    DeseralizationFailed(DeseralizationFailedError),
    StreamFailed(ReqwestError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSseFieldName =>
                write!(f, "the limited SSE parser only supports \"data\" field, an unsupported field name was received"),
            Self::ResponseExceededBuffer =>
                write!(f, "the response overflowed the streaming buffer, this could indicate a malicious server"),
            Self::DeseralizationFailed(e) => write!(f, "failed to deseralized a streamed JSON object \"{}\": {}", e.blob, e.error),
            Self::StreamFailed(e) => write!(f, "the source stream failed: {}", e),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::StreamFailed(e) => Some(e),
            Self::DeseralizationFailed(e) => Some(&e.error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct JsonStreamParser<S>
where
    S: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    stream: S,
    buf: Vec<u8>,
    max_size: usize,
    format: StreamFormat,
    i: usize,
    data: Vec<u8>,
}

impl<S: Stream<Item = reqwest::Result<Bytes>> + Unpin> JsonStreamParser<S> {
    pub(crate) fn new(stream: S, format: StreamFormat) -> JsonStreamParser<S> {
        Self::with_max_size_and_capacity(
            stream,
            format,
            1 << 24, // 4 MiB
            1 << 10, // 1 KiB
        )
    }

    pub(crate) fn with_max_size_and_capacity(
        stream: S,
        format: StreamFormat,
        max_size: usize,
        init_capacity: usize,
    ) -> JsonStreamParser<S> {
        JsonStreamParser {
            stream,
            buf: Vec::with_capacity(init_capacity),
            max_size,
            format,
            i: 0,
            data: Vec::<u8>::new(),
        }
    }

    async fn refill_buffer(&mut self) -> Result<bool, Error> {
        if let Some(b) = self.stream.next().await {
            match b {
                Ok(b) => {
                    if b.len() + self.buf.len() > self.max_size {
                        return Err(Error::ResponseExceededBuffer);
                    }

                    self.buf.extend(b);

                    Ok(true)
                }
                Err(err) => Err(Error::StreamFailed(err.into())),
            }
        } else {
            Ok(false)
        }
    }

    // Advance cursor to the next line
    //
    // Returns true when the operation completes successfully
    // and the cursor is positioned on the newline. Otherwise,
    // more data is needed
    fn advance_to_line(&mut self) -> bool {
        let i = &mut self.i;

        // Walk up to \n
        while *i < self.buf.len() && self.buf[*i] != b'\n' {
            *i += 1;
        }

        return *i != self.buf.len();
    }

    // Get line without the trailing [\r]\n
    fn striped_line(i: usize, buf: &Vec<u8>) -> &[u8] {
        if i == 0 {
            &buf[..0]
        } else if buf[i - 1] == b'\r' {
            &buf[..i - 1]
        } else {
            &buf[..i]
        }
    }

    // Extracts a line from the input (buf) and puts it
    // in the data buffer
    fn extract_json_line(&mut self) -> bool {
        loop {
            if !self.advance_to_line() {
                return false;
            }

            let line_content = Self::striped_line(self.i, &self.buf);

            self.data.extend_from_slice(line_content);

            self.buf.remove_first(self.i + 1);
            self.i = 0;

            if self.data.len() == 0 {
                continue;
            }

            return true;
        }
    }

    fn extract_lsse_data(&mut self) -> Result<bool, Error> {
        loop {
            if !self.advance_to_line() {
                return Ok(false);
            }

            let line_content = Self::striped_line(self.i, &self.buf);

            // Got data: CONTEXT, append to data buffer
            let end_of_event = if line_content.len() == 0 {
                // If there is no data, the event was just a comment
                Ok(self.data.len() > 0)
            } else {
                let mut split = line_content.splitn(2, |x| *x == b':');

                let field_name = split.next().unwrap();
                let value = split.next().unwrap_or_default();

                // Comment, skip
                if field_name.len() == 0 {
                    Ok(false)
                // Add to data buffer
                } else if field_name == b"data" {
                    // Remove the leading space (if it exists)
                    let value = value.strip_prefix(b" ").unwrap_or(value);

                    if value == b"[DONE]" {
                        // Skip terminal [DATA]
                        Ok(false)
                    } else {
                        self.data.extend_from_slice(value);
                        self.data.push(b'\n');

                        Ok(false)
                    }

                // Unknown field name
                } else {
                    Err(Error::UnsupportedSseFieldName)
                }
            };

            self.buf.remove_first(self.i + 1);
            self.i = 0;

            if !end_of_event? {
                continue;
            }

            // remove trailing \n
            if self.data.len() > 0 {
                self.data.pop();
            }

            return Ok(true);
        }
    }

    async fn parse_chunk<'d>(&'d mut self) -> Option<Result<&'d [u8], Error>> {
        // Clear the previous chunk
        self.data.clear();

        loop {
            let extracted = match self.format {
                StreamFormat::Ndjson => self.extract_json_line(),
                StreamFormat::LSSE => {
                    let extracted = self.extract_lsse_data();

                    if let Err(err) = extracted {
                        return Some(Err(err));
                    }

                    extracted.unwrap()
                }
            };

            if extracted {
                return Some(Ok(&self.data));
            }

            match self.refill_buffer().await {
                Ok(has_data) => {
                    if has_data {
                        continue;
                    } else {
                        break;
                    }
                }
                Err(err) => return Some(Err(err)),
            }
        }

        None
    }

    pub(crate) async fn parse<'de, T: Deserialize<'de>>(&'de mut self) -> Option<Result<T, Error>> {
        let c = self.parse_chunk().await;

        c.and_then(|r| {
            Some(match r {
                Ok(bytes) => serde_json::from_slice::<T>(&bytes).map_err(|e| {
                    Error::DeseralizationFailed(DeseralizationFailedError {
                        blob: String::from_utf8_lossy(bytes).into_owned(),
                        error: e,
                    })
                }),
                Err(err) => Err(err),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::result;

    use super::*;
    use futures_util::stream;

    const NDJSON_STREAM: &'static str = r#"
{"model":"gemma:2b","done":false}
{"model":"llama:7b","done":true}
"#;

    const LSEE_STREAM1: &'static str = r#"
data: {"model":"gemma:2b","done":false}

data:{"model":"llama:7b","done":true}

"#;

    const LSEE_STREAM2: &'static str = r#"
: Comment

data: {"model":"gemma:2b","done":false}

"#;

    const LSEE_STREAM3: &'static str = r#"
data: {"model":"gemma:2b","
data: done":false}

"#;

    const LSEE_STREAM4: &'static str = r#"
data: {"model":"gemma:2b",
data: "done":false}

"#;

    // This should cause an error (MalformattedStreamError::UnsupportedSseFieldName)
    const LSEE_STREAM5: &'static str = r#"
hello: {"model":"gemma:2b"," data: done":false}

"#;

    const LSSE_STREAM6: &'static str = r#"
data: {"model":"gemma:2b","done":false}

data: [DONE]

"#;

    fn stream_parser(
        chunk_size: usize,
        stream: &'static str,
        typ: StreamFormat,
    ) -> JsonStreamParser<
        futures_util::stream::Iter<std::vec::IntoIter<Result<bytes::Bytes, reqwest::Error>>>,
    > {
        let data = Bytes::from(stream);

        let data: Vec<Result<Bytes, reqwest::Error>> = data
            .chunks(chunk_size)
            .map(|c| Ok(Bytes::from(c.to_owned())))
            .collect();

        let stream = stream::iter(data);

        return JsonStreamParser::new(stream, typ);
    }

    #[tokio::test]
    async fn test_json_stream_chunking() {
        for chunk_size in 1..NDJSON_STREAM.len() {
            let mut parser = stream_parser(chunk_size, NDJSON_STREAM, StreamFormat::Ndjson);

            let chunk1 = parser.parse_chunk().await.unwrap().expect("should parse");

            assert_eq!(
                String::from_utf8(chunk1.to_vec()).unwrap(),
                r#"{"model":"gemma:2b","done":false}"#
            );

            let chunk2 = parser.parse_chunk().await.unwrap().expect("should parse");

            assert_eq!(
                String::from_utf8(chunk2.to_vec()).unwrap(),
                r#"{"model":"llama:7b","done":true}"#
            );

            let chunk3 = parser.parse_chunk().await;
            assert!(chunk3.is_none());
        }
    }

    #[derive(Debug, Deserialize)]
    struct ModelJson<'c> {
        model: &'c str,
        done: bool,
    }

    #[tokio::test]
    async fn test_json_stream_parser() {
        for chunk_size in 1..NDJSON_STREAM.len() {
            // Mock a stream to pass to the parser
            let mut parser = stream_parser(chunk_size, NDJSON_STREAM, StreamFormat::Ndjson);

            let result1 = parser.parse::<ModelJson>().await.unwrap();
            assert_eq!(result1.unwrap().model, "gemma:2b");

            let result2 = parser.parse::<ModelJson>().await.unwrap();
            assert_eq!(result2.unwrap().model, "llama:7b");

            let result3 = parser.parse::<ModelJson>().await;
            assert!(result3.is_none());
        }
    }

    #[tokio::test]
    async fn test_stream_parse() {
        for chunk_size in 1..=10 {
            // LSSE_STREAM1
            {
                let mut parser = stream_parser(chunk_size, LSEE_STREAM1, StreamFormat::LSSE);

                let result = parser.parse::<ModelJson>().await.unwrap();
                assert!(result.is_ok());

                let result = result.unwrap();
                assert_eq!(result.model, "gemma:2b");

                let result = parser.parse::<ModelJson>().await.unwrap();
                assert!(result.is_ok());

                let result = result.unwrap();
                assert_eq!(result.model, "llama:7b");

                let result = parser.parse::<ModelJson>().await;
                assert!(result.is_none());
            }

            {
                let mut parser = stream_parser(chunk_size, LSEE_STREAM2, StreamFormat::LSSE);

                let result = parser.parse::<ModelJson>().await.unwrap();
                assert!(result.is_ok());
                let result = result.unwrap();
                assert_eq!(result.model, "gemma:2b");

                let result = parser.parse::<ModelJson>().await;
                assert!(result.is_none());
            }

            {
                let mut parser = stream_parser(chunk_size, LSEE_STREAM3, StreamFormat::LSSE);

                let result = parser.parse::<ModelJson>().await.unwrap();

                // Newline injected between json
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    Error::DeseralizationFailed(_)
                ));
            }

            {
                let mut parser = stream_parser(chunk_size, LSEE_STREAM4, StreamFormat::LSSE);

                let result = parser.parse::<ModelJson>().await.unwrap();
                assert!(result.is_ok());
                let result = result.unwrap();
                assert_eq!(result.model, "gemma:2b");

                let result = parser.parse::<ModelJson>().await;
                assert!(result.is_none());
            }

            {
                let mut parser = stream_parser(chunk_size, LSSE_STREAM6, StreamFormat::LSSE);

                let result = parser.parse::<ModelJson>().await.unwrap();
                assert!(result.is_ok());
                let result = result.unwrap();
                assert_eq!(result.model, "gemma:2b");

                let result = parser.parse::<ModelJson>().await;
                assert!(result.is_none());
            }

            {
                let mut parser = stream_parser(chunk_size, LSEE_STREAM5, StreamFormat::LSSE);

                let result = parser.parse::<ModelJson>().await.unwrap();
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    Error::UnsupportedSseFieldName
                ));
            }
        }
    }
}
