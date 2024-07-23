//! Wrapper around Reqwest's error type to facilitate exclusive matching

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorKind {
    ConnectFailed,
    DecodingFailed,
    RedirectPolicyViolated,
    TimedOut,
    UnknownReqwestError,
}

#[derive(Debug)]
pub(crate) struct Error {
    kind: ErrorKind,
    source: Option<reqwest::Error>,
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::new(err)
    }
}

impl Error {
    pub(crate) fn new(err: reqwest::Error) -> Error {
        let kind = if err.is_decode() {
            ErrorKind::DecodingFailed
        } else if err.is_timeout() {
            ErrorKind::TimedOut
        } else if err.is_redirect() {
            ErrorKind::RedirectPolicyViolated
        } else if err.is_connect() {
            ErrorKind::ConnectFailed
        } else {
            ErrorKind::UnknownReqwestError
        };

        Error {
            kind,
            source: Some(err),
        }
    }

    pub(crate) fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::ConnectFailed => write!(f, "connection failed"),
            ErrorKind::DecodingFailed => write!(f, "decoding failed"),
            ErrorKind::RedirectPolicyViolated => write!(f, "redirect policy violated"),
            ErrorKind::TimedOut => write!(f, "timed out"),
            ErrorKind::UnknownReqwestError => write!(f, "unknown reqwest error"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|e| e as &(dyn StdError + 'static))
    }
}
