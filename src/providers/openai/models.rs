use lazy_static::lazy_static;

use crate::providers::Model;

lazy_static! {
    // The OpenAI API does not include an API route to list their active models. This
    // limits release stability (since any of chat model could be deprecated and pulled.)
    // It also means that this list needs to be updated whenever new models are added or
    // the context length of a model changes.
    pub(super) static ref OPENAI_MODELS: [Model; 5] = [
        Model {
            id: "gpt-4o-mini".to_string(),
            context_length: Some(128000),
        },
        Model {
            id: "gpt-4o".to_string(),
            context_length: Some(128000),
        },
        Model {
            id: "gpt-4-turbo".to_string(),
            context_length: Some(128000),
        },
        Model {
            id: "gpt-4".to_string(),
            context_length: Some(8192),
        },
        Model {
            id: "gpt-3.5-turbo".to_string(),
            context_length: Some(16385)
        },
    ];

    // This is the default model unless it is overridden by the user.
    // This should default to the cheepest flagship model.
    pub(super) static ref DEFAULT_MODEL: &'static Model = &OPENAI_MODELS[0];
}
