use super::{JsonStreamParser, StreamFormat};
use futures_util::Stream;
use std::marker::Unpin;

pub(crate) trait ReqwestResponseStreamExt {
    fn stream_ndjson(
        self,
    ) -> JsonStreamParser<impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin>;
    fn stream_lsse(
        self,
    ) -> JsonStreamParser<impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin>;
}

impl ReqwestResponseStreamExt for reqwest::Response {
    fn stream_lsse(
        self,
    ) -> JsonStreamParser<impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin> {
        JsonStreamParser::new(self.bytes_stream(), StreamFormat::LSSE)
    }

    fn stream_ndjson(
        self,
    ) -> JsonStreamParser<impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin> {
        JsonStreamParser::new(self.bytes_stream(), StreamFormat::Ndjson)
    }
}
