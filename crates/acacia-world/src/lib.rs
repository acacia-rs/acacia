//! Network-free world state for Bedrock 1.26.5x bots: the block registry (runtime ids, collision,
//! physics flags), chunk decoding, and chunk storage shared between bots. See README.md.

mod access;
mod change;
mod chunk;
mod registry;
mod view;
mod world;

pub use access::BlockAccess;
pub use change::ChunkChange;
pub use chunk::{
    Chunk, ColumnHeights, Dimension, Heightmap, LevelChunkData, SECTION_VOLUME, SubChunkData, level_chunk_block_entities,
    sub_chunk_block_entities,
};
pub use registry::{
    Aabb, BlockFlags, BlockRegistry, BlockState, CustomBlock, Material, Mining, Tool, ToolKind, ToolTier, fnv1_64,
};
pub use view::ChunkView;
pub use world::{BlockIds, Inserted, SharedChunk, World};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("chunk data ended early")]
    UnexpectedEof,
    #[error("varint longer than 5 bytes")]
    VarIntTooLong,
    #[error("unsupported sub-chunk version {0}")]
    SectionVersion(u8),
    #[error("persistent (NBT) palette in a network chunk")]
    PersistentPalette,
    #[error("invalid bits per block {0}")]
    BitsPerBlock(u8),
    #[error("invalid palette length {0}")]
    PaletteLength(i32),
}
