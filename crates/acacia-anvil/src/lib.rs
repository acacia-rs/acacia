//! Bedrock chunks held by `acacia-world` written out as a Java Edition Anvil world (see README.md).

pub mod column;
pub mod encode;
pub mod light;
pub mod mapping;
pub mod nbt;
pub mod pack;
pub mod props;
pub mod region;
pub mod section;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("compressed chunk of {0} bytes does not fit a region slot")]
    ChunkTooLarge(usize),
}
