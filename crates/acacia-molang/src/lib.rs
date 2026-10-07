//! Molang, the expression language of Minecraft: Bedrock Edition packs. See the README.

mod alike;
mod compiler;
mod error;
mod eval;
mod exec;
mod fold;
mod host;
mod lex;
mod math;
mod names;
mod optimise;
mod parse;
mod parse_array;
mod parse_expr;
mod parse_name;
mod post;
mod program;
mod queries;
mod store;
mod sum;
mod value;

pub use compiler::{Arrays, Compiler, Context, Engine, Query, Variable};
pub use error::{Error, ErrorKind};
pub use host::{Env, Host, NoHost, Scratch, Structs, Variables};
pub use program::Program;
pub use queries::QUERY_NAMES;
pub use value::{StructRef, Symbol, Value};
