//! Bedrock chunks held by `acacia-world` written out as a Java Edition Anvil world (see README.md).

pub mod blockentity;
pub mod column;
pub mod decode;
pub mod dimension;
pub mod encode;
pub mod fixup;
pub mod light;
pub mod mapping;
pub mod nbt;
pub mod pack;
pub mod props;
pub mod region;
pub mod section;
pub mod store;

pub use store::{AnvilStore, FlushStats, StoreConfig, region_dir};

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("compressed chunk of {0} bytes does not fit a region slot")]
    ChunkTooLarge(usize),
}
