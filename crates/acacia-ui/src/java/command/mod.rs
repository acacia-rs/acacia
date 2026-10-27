//! The client's command tree as Brigadier 1.3.11 reads it, for the chat screen's
//! `CommandSuggestions`: parsing the input into nodes and argument ranges, the errors of what does
//! not parse, completions and usage. Ported from Mojang's Brigadier (MIT); only the argument types
//! the viewer maps Bedrock's parameters to, and no commands, requirements or redirects.

pub mod error;
pub mod parse;
pub mod reader;
pub mod suggest;
pub mod tree;
mod usage;

pub use error::Error;
pub use parse::{Parse, Range};
pub use suggest::{Suggestion, Suggestions};
pub use tree::{ArgType, NodeKind, Tree};

#[cfg(test)]
mod tests;
