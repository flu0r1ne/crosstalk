//! Conversions between Reqwest API errors and provider error types

use crate::providers::{Error, ErrorKind};
use crate::providers::apireq::{JsonStreamError, ReqwestError, error::ErrorKind as ReqwestErrorKind};

impl From<JsonStreamError> for Error {
    fn from(value: JsonStreamError) -> Self {
        let kind = match &value {
            JsonStreamError::DeseralizationFailed(_)
            | JsonStreamError::UnsupportedSseFieldName
            | JsonStreamError::ResponseExceededBuffer => ErrorKind::UnexpectedResponse,
            // This might fit better as "unexpected response"
            JsonStreamError::StreamFailed(_) => ErrorKind::UnspecifiedError,
        };

        Error::from_source(kind, Box::new(value))
    }
}

impl From<ReqwestError> for Error {
    fn from(value: ReqwestError) -> Self {
        let kind: ErrorKind = match &value.kind() {
            ReqwestErrorKind::ConnectFailed => ErrorKind::Connection,
            ReqwestErrorKind::DecodingFailed | ReqwestErrorKind::RedirectPolicyViolated => {
                ErrorKind::UnexpectedResponse
            }
            ReqwestErrorKind::TimedOut => ErrorKind::TimedOut,
            ReqwestErrorKind::UnknownReqwestError => ErrorKind::UnspecifiedError,
        };

        Error::from_source(kind, Box::new(value))
    }
}
