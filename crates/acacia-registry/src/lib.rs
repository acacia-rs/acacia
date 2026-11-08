//! Edition-neutral blocks: one `Block` kind and `BlockState` for Java and Bedrock (README.md).

mod block;
mod generated;
pub mod prop;
mod state;
mod tag;

/// Property value types shared across properties (`Direction` for `FACING`, `HORIZONTAL_FACING`...).
pub mod value {
    pub use crate::generated::values::*;
}

pub use block::{Block, ParseBlockError};
pub use state::{BlockState, ParseStateError};
pub use tag::BlockTag;
