//! Edition-neutral blocks and items: one `Block`, `BlockState` and `Item` for Java and Bedrock (README.md).

mod block;
mod collision;
mod generated;
mod item;
pub mod prop;
mod state;
mod tag;

/// Property value types shared across properties (`Direction` for `FACING`, `HORIZONTAL_FACING`...).
pub mod value {
    pub use crate::generated::values::*;
}

pub use block::{Block, ParseBlockError};
pub use collision::CollisionBox;
pub use item::{Item, ParseItemError};
pub use state::{BlockState, ParseStateError};
pub use tag::{BlockTag, ItemTag};
