//! Java's block and item tags: `Block::OakFence.is(BlockTag::Fences)`, `Item::OakLog.is(ItemTag::Logs)`.

use crate::block::Block;
pub use crate::generated::block_tags::BlockTag;
pub use crate::generated::item_tags::ItemTag;
use crate::generated::{block_tags, item_tags};
use crate::item::Item;

pub(crate) struct Tag<const W: usize> {
    pub name: &'static str,
    pub bits: [u64; W],
}

impl<const W: usize> Tag<W> {
    fn has(&self, i: usize) -> bool {
        self.bits[i >> 6] >> (i & 63) & 1 == 1
    }
}

impl BlockTag {
    /// Namespaced, e.g. `minecraft:mineable/pickaxe`.
    pub fn name(self) -> &'static str {
        block_tags::TAGS[self as usize].name
    }

    /// Bedrock-only blocks belong to no tag.
    pub fn contains(self, block: Block) -> bool {
        block_tags::TAGS[self as usize].has(block as usize)
    }

    pub fn blocks(self) -> impl Iterator<Item = Block> {
        Block::all().filter(move |b| self.contains(*b))
    }
}

impl ItemTag {
    pub fn name(self) -> &'static str {
        item_tags::TAGS[self as usize].name
    }

    pub fn contains(self, item: Item) -> bool {
        item_tags::TAGS[self as usize].has(item as usize)
    }

    pub fn items(self) -> impl Iterator<Item = Item> {
        Item::all().filter(move |i| self.contains(*i))
    }
}

impl Block {
    pub fn is(self, tag: BlockTag) -> bool {
        tag.contains(self)
    }
}

impl Item {
    pub fn is(self, tag: ItemTag) -> bool {
        tag.contains(self)
    }
}
