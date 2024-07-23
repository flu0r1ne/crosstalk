//! A utility model with helpers for making and parsing API requests.

mod error;
mod json_stream_parser;
mod provider;
mod stream_ext;

pub(crate) use error::Error as ReqwestError;
pub(crate) use reqwest::Url;

pub(crate) use json_stream_parser::Error as JsonStreamError;
pub(crate) use json_stream_parser::JsonStreamParser;
pub(crate) use json_stream_parser::StreamFormat;
pub(crate) use stream_ext::ReqwestResponseStreamExt;
