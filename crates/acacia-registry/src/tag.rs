//! Java's block tags: `Block::OakFence.is(BlockTag::Fences)`.

use crate::block::Block;
pub use crate::generated::tags::BlockTag;
use crate::generated::tags::{TAG_WORDS, TAGS};

pub(crate) struct Tag {
    pub name: &'static str,
    pub bits: [u64; TAG_WORDS],
}

impl BlockTag {
    /// Namespaced, e.g. `minecraft:mineable/pickaxe`.
    pub fn name(self) -> &'static str {
        TAGS[self as usize].name
    }

    pub fn contains(self, block: Block) -> bool {
        let i = block as usize;
        TAGS[self as usize].bits[i >> 6] >> (i & 63) & 1 == 1
    }

    pub fn blocks(self) -> impl Iterator<Item = Block> {
        Block::all().filter(move |b| self.contains(*b))
    }
}

impl Block {
    pub fn is(self, tag: BlockTag) -> bool {
        tag.contains(self)
    }
}
