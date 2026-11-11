use std::fmt;
use std::ops::Range;
use std::str::FromStr;

pub use crate::generated::blocks::Block;
use crate::generated::blocks::{BEDROCK_ALIASES, BLOCKS, JAVA_BLOCK_COUNT};
use crate::prop::PropId;
use crate::state::BlockState;

pub(crate) struct BlockInfo {
    pub block: Block,
    pub name: &'static str,
    pub first: u32,
    pub default: u32,
    /// In state order: the last property varies fastest.
    pub props: &'static [PropId],
}

impl Block {
    pub fn all() -> impl ExactSizeIterator<Item = Block> {
        BLOCKS.iter().map(|b| b.block)
    }

    pub(crate) fn info(self) -> &'static BlockInfo {
        &BLOCKS[self as usize]
    }

    /// Namespaced, e.g. `minecraft:oak_log`. Java's name, or Bedrock's for a Bedrock-only block.
    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn default_state(self) -> BlockState {
        BlockState(self.info().default)
    }

    pub fn states(self) -> impl ExactSizeIterator<Item = BlockState> {
        self.state_range().map(BlockState)
    }

    pub(crate) fn state_range(self) -> Range<u32> {
        let first = self.info().first;
        first..first + self.info().props.iter().map(|p| p.count()).product::<u32>()
    }

    pub fn properties(self) -> &'static [PropId] {
        self.info().props
    }

    /// Whether Java Edition has this block; the rest exist only on Bedrock.
    pub fn is_java(self) -> bool {
        (self as usize) < JAVA_BLOCK_COUNT
    }

    /// Accepts either edition's spelling, with or without `minecraft:`.
    pub fn from_name(name: &str) -> Option<Block> {
        let path = name.strip_prefix("minecraft:").unwrap_or(name);
        let full = |n: &str| n.strip_prefix("minecraft:").unwrap_or(n) == path;
        if let Some(info) = BLOCKS.iter().find(|b| full(b.name)) {
            return Some(info.block);
        }
        BEDROCK_ALIASES.binary_search_by(|(n, _)| n[10..].cmp(path)).ok().map(|i| BEDROCK_ALIASES[i].1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBlockError(pub String);

impl fmt::Display for ParseBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown block `{}`", self.0)
    }
}

impl std::error::Error for ParseBlockError {}

impl FromStr for Block {
    type Err = ParseBlockError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Block::from_name(s).ok_or_else(|| ParseBlockError(s.to_owned()))
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
