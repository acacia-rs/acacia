use std::fmt;
use std::str::FromStr;

use crate::block::Block;
use crate::generated::blocks::{BLOCKS, JAVA_STATE_COUNT, STATE_COUNT};
use crate::prop::{PropId, Property};

/// A neutral block state. Ids below `JAVA_STATE_COUNT` are Java 26.3's global palette ids, so a Java
/// chunk needs no translation; Bedrock-only blocks follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockState(pub(crate) u32);

impl BlockState {
    pub const AIR: BlockState = BlockState(0);

    pub fn from_id(id: u32) -> Option<BlockState> {
        (id < STATE_COUNT).then_some(BlockState(id))
    }

    pub fn from_java_id(id: u32) -> Option<BlockState> {
        (id < JAVA_STATE_COUNT).then_some(BlockState(id))
    }

    pub fn id(self) -> u32 {
        self.0
    }

    pub fn java_id(self) -> Option<u32> {
        (self.0 < JAVA_STATE_COUNT).then_some(self.0)
    }

    pub fn block(self) -> Block {
        let i = BLOCKS.partition_point(|b| b.first <= self.0) - 1;
        BLOCKS[i].block
    }

    pub fn is_air(self) -> bool {
        matches!(self.block(), Block::Air | Block::CaveAir | Block::VoidAir)
    }

    /// `None` when the block has no such property.
    pub fn get<P: Property>(self, _: P) -> Option<P::Value> {
        self.digit(P::ID).map(|(digit, _)| P::VALUES[digit as usize])
    }

    /// `None` when the block has no such property or the property can't take `value`.
    pub fn with<P: Property>(self, _: P, value: P::Value) -> Option<BlockState> {
        let (digit, stride) = self.digit(P::ID)?;
        let new = P::VALUES.iter().position(|v| *v == value)? as u32;
        Some(BlockState(self.0 - digit * stride + new * stride))
    }

    /// Each property with its serialized value, in state order.
    pub fn properties(self) -> impl Iterator<Item = (PropId, &'static str)> {
        let block = self.block();
        let props = block.properties();
        let mut rest = self.0 - block.info().first;
        let mut digits = vec![0; props.len()];
        for (i, p) in props.iter().enumerate().rev() {
            digits[i] = rest % p.count();
            rest /= p.count();
        }
        props.iter().zip(digits).map(|(p, d)| (*p, p.values()[d as usize]))
    }

    /// (digit, stride) of `prop` within this state's id.
    fn digit(self, prop: PropId) -> Option<(u32, u32)> {
        let info = self.block().info();
        let mut rest = self.0 - info.first;
        let mut stride = 1;
        for p in info.props.iter().rev() {
            let n = p.count();
            if *p == prop {
                return Some((rest % n, stride));
            }
            rest /= n;
            stride *= n;
        }
        None
    }

    /// The state of `block` with the given `key=value` pairs; unnamed properties keep their default.
    pub fn with_named(block: Block, pairs: &[(&str, &str)]) -> Result<BlockState, ParseStateError> {
        let mut state = block.default_state();
        for (key, value) in pairs {
            let prop = block.properties().iter().find(|p| p.key() == *key).ok_or_else(|| ParseStateError::Property {
                block,
                key: (*key).to_owned(),
            })?;
            let new = prop.values().iter().position(|v| v == value).ok_or_else(|| ParseStateError::Value {
                block,
                key: (*key).to_owned(),
                value: (*value).to_owned(),
            })? as u32;
            let (digit, stride) = state.digit(*prop).expect("property of this block");
            state = BlockState(state.0 - digit * stride + new * stride);
        }
        Ok(state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseStateError {
    Syntax(String),
    Block(String),
    Property { block: Block, key: String },
    Value { block: Block, key: String, value: String },
}

impl fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "malformed block state `{s}`"),
            Self::Block(s) => write!(f, "unknown block `{s}`"),
            Self::Property { block, key } => write!(f, "{block} has no property `{key}`"),
            Self::Value { block, key, value } => write!(f, "{block} property `{key}` can't be `{value}`"),
        }
    }
}

impl std::error::Error for ParseStateError {}

/// `minecraft:oak_stairs[facing=east,half=top]`, Java's syntax.
impl FromStr for BlockState {
    type Err = ParseStateError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, rest) = s.split_once('[').map_or((s, None), |(n, r)| (n, Some(r)));
        let block = Block::from_name(name).ok_or_else(|| ParseStateError::Block(name.to_owned()))?;
        let mut pairs = Vec::new();
        if let Some(rest) = rest {
            let body = rest.strip_suffix(']').ok_or_else(|| ParseStateError::Syntax(s.to_owned()))?;
            for pair in body.split(',').filter(|p| !p.is_empty()) {
                pairs.push(pair.split_once('=').ok_or_else(|| ParseStateError::Syntax(s.to_owned()))?);
            }
        }
        BlockState::with_named(block, &pairs)
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.block().name())?;
        let mut first = true;
        for (prop, value) in self.properties() {
            f.write_str(if first { "[" } else { "," })?;
            first = false;
            write!(f, "{}={value}", prop.key())?;
        }
        if !first {
            f.write_str("]")?;
        }
        Ok(())
    }
}

impl From<Block> for BlockState {
    fn from(block: Block) -> Self {
        block.default_state()
    }
}
