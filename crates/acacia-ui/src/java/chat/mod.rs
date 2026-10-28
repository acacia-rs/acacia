//! Chat as Java 26.3 draws it: the message list (`ChatComponent`) and the chat screen's input
//! line with its suggestions (`ChatScreen`, `EditBox`, `CommandSuggestions`).

mod command_info;
pub mod component;
mod component_draw;
pub mod edit_box;
mod edit_box_draw;
pub mod screen;
pub mod suggestions;
pub mod suggestions_list;

pub use component::{Chat, ChatOptions, Tag, TagText};
pub use component_draw::Focus;
pub use edit_box::{EditBox, Key};
pub use screen::{ChatAction, ChatLines, ChatScreen};
pub use suggestions::Completion;

#[cfg(test)]
mod tests;
