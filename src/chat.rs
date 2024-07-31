//! Type definitions for chat primitives
//!

/// The author of a `Message`
#[derive(Debug, Clone)]
pub(crate) enum Role {
    /// A `System` message is an authoritative message which is used to
    /// instruct the model. Usually, it appears as the first message
    /// in a dialog.
    System,

    /// A message authored by the user
    User,

    /// A message authored by the model
    Model,

    /// A message that is not included in the chat dialog. (That is, it
    /// is never shown to the model.) `Info` messages are produced in
    /// response to user commands.
    Info,
}

/// A `Message` in a chat converstation
#[derive(Debug, Clone)]
pub(crate) struct Message {
    /// The author of the message
    pub role: Role,
    /// The contents of the message
    pub content: String,
}

impl Message {
    pub(crate) fn new(role: Role, content: String) -> Message {
        Message { role, content }
    }
}
