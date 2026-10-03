//! Block states in runtime-id order for 1.26.50+ (Geyser `block_palette.26_50.nbt`).
//!
//! Runtime id = index into the palette sorted by FNV-1 64 of the block name (stable, so states of
//! one block keep their canonical order). Custom blocks from `StartGame.block_properties` are merged
//! into that sort; see [`BlockRegistry::with_custom_blocks`].

mod blob;
mod mining;
mod state;

use std::sync::{Arc, LazyLock, OnceLock};

use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};

pub use mining::{Material, Mining, Tool, ToolKind, ToolTier};
pub use state::{Aabb, BlockFlags, BlockState};

#[derive(Debug)]
pub struct BlockRegistry {
    states: Vec<BlockState>,
    air: u32,
    by_hash: OnceLock<FxHashMap<u32, u32>>,
}

/// A block from `StartGame.block_properties`. `state_count` is the product of the value counts of
/// its `properties[].enum` lists (1 when it has none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBlock {
    pub name: String,
    pub state_count: u32,
}

static VANILLA: LazyLock<Arc<BlockRegistry>> =
    LazyLock::new(|| Arc::new(BlockRegistry::from_states(blob::vanilla_states())));

impl BlockRegistry {
    pub fn vanilla() -> &'static BlockRegistry {
        &VANILLA
    }

    pub fn vanilla_arc() -> Arc<BlockRegistry> {
        VANILLA.clone()
    }

    fn from_states(states: Vec<BlockState>) -> Self {
        let air = states
            .iter()
            .position(|s| s.name == "minecraft:air")
            .expect("palette has minecraft:air") as u32;
        BlockRegistry { states, air, by_hash: OnceLock::new() }
    }

    /// Runtime id for a hashed network id (see [`BlockState::network_hash`]).
    pub fn runtime_id_from_hash(&self, hash: u32) -> Option<u32> {
        let map = self.by_hash.get_or_init(|| {
            let hashed = self.states.iter().enumerate().filter(|(_, s)| s.network_hash != 0);
            hashed.map(|(i, s)| (s.network_hash, i as u32)).collect()
        });
        map.get(&hash).copied()
    }

    /// Registry with custom blocks merged in the client's order. Names already present are skipped:
    /// since 26.50 servers also list data-driven vanilla blocks (wool stairs etc.) in
    /// `block_properties`, and those are already part of the vanilla palette.
    /// Custom states have no properties and collide as full cubes.
    pub fn with_custom_blocks(&self, blocks: &[CustomBlock]) -> BlockRegistry {
        let mut states = self.states.clone();
        for b in self.new_blocks(blocks) {
            let name = intern(&b.name);
            states.extend((0..b.state_count.max(1)).map(|_| custom_state(name)));
        }
        states.sort_by_key(|s| fnv1_64(s.name));
        BlockRegistry::from_states(states)
    }

    /// The air runtime id [`BlockRegistry::with_custom_blocks`] would have, without building it.
    pub fn air_id_with_custom_blocks(&self, blocks: &[CustomBlock]) -> u32 {
        let air = fnv1_64("minecraft:air");
        let before = self.new_blocks(blocks).filter(|b| fnv1_64(&b.name) < air);
        self.air + before.map(|b| b.state_count.max(1)).sum::<u32>()
    }

    fn new_blocks<'a>(&self, blocks: &'a [CustomBlock]) -> impl Iterator<Item = &'a CustomBlock> {
        let known: FxHashSet<&str> = self.states.iter().map(|s| s.name).collect();
        blocks.iter().filter(move |b| !known.contains(b.name.as_str()))
    }

    pub fn get(&self, runtime_id: u32) -> Option<&BlockState> {
        self.states.get(runtime_id as usize)
    }

    pub fn air_id(&self) -> u32 {
        self.air
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Linear scan; `properties` is `k=v` pairs in any order (`""` for none).
    pub fn find(&self, name: &str, properties: &str) -> Option<u32> {
        let mut want: Vec<&str> = properties.split(',').filter(|s| !s.is_empty()).collect();
        want.sort_unstable();
        let want = want.join(",");
        self.states
            .iter()
            .position(|s| s.name == name && s.properties == want)
            .map(|i| i as u32)
    }

    /// All `(runtime_id, state)` of one block.
    pub fn states_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = (u32, &'a BlockState)> {
        self.states
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.name == name)
            .map(|(i, s)| (i as u32, s))
    }
}

fn custom_state(name: &'static str) -> BlockState {
    static FULL: [Aabb; 1] = [Aabb::FULL];
    BlockState {
        name,
        properties: "",
        boxes: &FULL,
        friction: 0.6,
        flags: BlockFlags::FULL_CUBE,
        liquid_depth: 0,
        network_hash: 0,
        mining: Mining::UNKNOWN,
        light_emission: 0,
        light_filter: 15,
    }
}

/// Custom block names live as long as the process; each distinct name is leaked once.
fn intern(name: &str) -> &'static str {
    static NAMES: LazyLock<Mutex<FxHashSet<&'static str>>> = LazyLock::new(Default::default);
    let mut names = NAMES.lock();
    if let Some(n) = names.get(name) {
        return n;
    }
    let leaked: &'static str = Box::leak(name.into());
    names.insert(leaked);
    leaked
}

/// FNV-1 (multiply, then xor) 64-bit, the block-name sort key of the client and Geyser.
pub fn fnv1_64(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h: u64, b| {
        h.wrapping_mul(0x0000_0100_0000_01b3) ^ b as u64
    })
}
