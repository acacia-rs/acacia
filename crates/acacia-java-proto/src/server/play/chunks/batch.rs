//! Chunk batching and the chunk cache window.

#![allow(unused_doc_comments)] // docs on `packet!` fields' structs

use crate::packets::packet;
use crate::{Error, VarInt, Wire};

pub use crate::packets::{ChunkBatchReceived, Empty as ChunkBatchStart};

packet!(ChunkBatchFinished { batch_size: VarInt });
/// The chunk the player is in; VarInt coordinates.
packet!(SetChunkCacheCenter { x: VarInt, z: VarInt });
packet!(SetChunkCacheRadius { radius: VarInt });
