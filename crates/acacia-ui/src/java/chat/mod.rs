//! Chat as Java 26.3 draws it: the message list (`ChatComponent`) and the chat screen's input
//! line with its command suggestions (`ChatScreen`, `EditBox`, `CommandSuggestions`).

pub mod component;

pub use component::{Chat, ChatOptions, Tag};

#[cfg(test)]
mod tests;
