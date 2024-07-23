//! An unbrella module for the OpenAI provider

mod api;
mod models;
mod provider;

pub(crate) use self::provider::OpenAIProvider;
