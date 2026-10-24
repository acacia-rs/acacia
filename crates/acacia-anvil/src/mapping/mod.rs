//! Java 26.3 block states and the Bedrock → Java maps, generated from Geyser's mappings
//! (tools/gen-mapping.mjs; ambiguities in README.md "Mapping").

mod blob;

use std::collections::HashMap;
use std::sync::OnceLock;

use acacia_world::BlockRegistry;

/// A Java block state: its id in the vanilla 26.3 state order (what Geyser's mappings are indexed by).
pub type JavaState = u16;

pub const AIR: JavaState = 0;

/// A property name and its values in enumeration order.
type Property = (&'static str, Box<[&'static str]>);

pub struct JavaBlock {
    pub name: &'static str,
    base: u32,
    default_index: u32,
    /// In state-enumeration order: the last property varies fastest.
    props: Box<[Property]>,
}

impl JavaBlock {
    fn state_count(&self) -> u32 {
        self.props.iter().map(|(_, v)| v.len() as u32).product()
    }

    pub fn default_state(&self) -> JavaState {
        (self.base + self.default_index) as JavaState
    }

    pub fn has_property(&self, key: &str) -> bool {
        self.props.iter().any(|(k, _)| *k == key)
    }
}

struct Tables {
    blocks: Vec<JavaBlock>,
    state_count: u32,
    bedrock: Vec<(&'static str, &'static str, u32)>,
    /// Per Java state: index into `bedrock` of the state Geyser sends for it.
    forward: Vec<u16>,
    /// Per Java state, from Java itself: dampening, emission, occluding faces, flags (props.rs).
    light: Vec<[u8; 4]>,
    biomes: Vec<(u32, &'static str)>,
}

pub struct JavaBlocks {
    t: Tables,
    by_name: HashMap<&'static str, u32>,
    /// Block index per state.
    block_of: Vec<u16>,
}

pub fn java() -> &'static JavaBlocks {
    static J: OnceLock<JavaBlocks> = OnceLock::new();
    J.get_or_init(|| {
        let t = blob::parse(include_bytes!("../../data/mapping.bin"));
        let by_name = t
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.name, i as u32))
            .collect();
        let mut block_of = Vec::with_capacity(t.state_count as usize);
        for (i, b) in t.blocks.iter().enumerate() {
            block_of.extend(std::iter::repeat_n(i as u16, b.state_count() as usize));
        }
        JavaBlocks {
            t,
            by_name,
            block_of,
        }
    })
}

impl JavaBlocks {
    pub fn state_count(&self) -> usize {
        self.t.state_count as usize
    }

    pub fn block(&self, state: JavaState) -> &JavaBlock {
        &self.t.blocks[self.block_of[state as usize] as usize]
    }

    pub fn find_block(&self, name: &str) -> Option<&JavaBlock> {
        self.by_name.get(name).map(|&i| &self.t.blocks[i as usize])
    }

    pub fn is_default(&self, state: JavaState) -> bool {
        self.block(state).default_state() == state
    }

    /// `(key, value)` pairs of a state, in the block's enumeration order.
    pub fn properties(
        &self,
        state: JavaState,
    ) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        let b = self.block(state);
        let mut rest = state as u32 - b.base;
        let mut picked = [0u8; 16];
        for (i, (_, vals)) in b.props.iter().enumerate().rev() {
            picked[i] = (rest % vals.len() as u32) as u8;
            rest /= vals.len() as u32;
        }
        b.props
            .iter()
            .enumerate()
            .map(move |(i, (k, vals))| (*k, vals[picked[i] as usize]))
    }

    /// `[light dampening, emission, occluding faces, flags]` as Java computes them (props.rs).
    pub fn light_data(&self, state: JavaState) -> [u8; 4] {
        self.t.light[state as usize]
    }

    pub fn property(&self, state: JavaState, key: &str) -> Option<&'static str> {
        self.properties(state)
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    /// The same block with `key` set to `value`; `None` if the block lacks that property or value.
    pub fn with(&self, state: JavaState, key: &str, value: &str) -> Option<JavaState> {
        let b = self.block(state);
        let mut stride = 1u32;
        for (k, vals) in b.props.iter().rev() {
            let n = vals.len() as u32;
            if *k == key {
                let new = vals.iter().position(|v| *v == value)? as u32;
                let old = (state as u32 - b.base) / stride % n;
                return Some((state as u32 - old * stride + new * stride) as JavaState);
            }
            stride *= n;
        }
        None
    }

    /// A block's state from a partial property list; unlisted properties keep their defaults.
    pub fn state(&self, name: &str, props: &[(&str, &str)]) -> Option<JavaState> {
        let mut s = self.find_block(name)?.default_state();
        for (k, v) in props {
            s = self.with(s, k, v)?;
        }
        Some(s)
    }
}

/// Bedrock runtime ids of one registry mapped to Java states, and Bedrock biome ids to Java biomes.
pub struct StateMap {
    to_java: Vec<JavaState>,
    to_bedrock: Vec<u32>,
    /// Runtime ids with no Geyser mapping (custom or Bedrock-only blocks), written as air.
    pub unmapped: Vec<u32>,
}

impl StateMap {
    pub fn new(registry: &BlockRegistry) -> Self {
        let j = java();
        let mut by_bedrock: HashMap<(&str, &str), JavaState> =
            HashMap::with_capacity(j.t.bedrock.len());
        for &(name, props, java) in &j.t.bedrock {
            by_bedrock.insert((name, props), java as JavaState);
        }
        let mut unmapped = Vec::new();
        let to_java = (0..registry.len() as u32)
            .map(|id| {
                let Some(s) = registry.get(id) else {
                    return AIR;
                };
                by_bedrock
                    .get(&(s.name, s.properties))
                    .copied()
                    .unwrap_or_else(|| {
                        unmapped.push(id);
                        nearest_mapped(registry, &by_bedrock, s.name, s.properties)
                            .or_else(|| {
                                j.find_block(&java_name_of_custom(s.name))
                                    .map(JavaBlock::default_state)
                            })
                            .unwrap_or(AIR)
                    })
            })
            .collect();
        let air = registry.air_id();
        let to_bedrock =
            j.t.forward
                .iter()
                .map(|&i| {
                    let (name, props, _) = j.t.bedrock[i as usize];
                    registry.find(name, props).unwrap_or(air)
                })
                .collect();
        Self {
            to_java,
            to_bedrock,
            unmapped,
        }
    }

    pub fn java(&self, runtime_id: u32) -> JavaState {
        self.to_java
            .get(runtime_id as usize)
            .copied()
            .unwrap_or(AIR)
    }

    /// The runtime id Geyser would send for a Java state (where per-state light and shape data come from).
    pub fn bedrock(&self, state: JavaState) -> u32 {
        self.to_bedrock[state as usize]
    }
}

/// Geyser sends some Java blocks as its own custom blocks (`geyser_custom:obsidian`, the huge mushroom
/// blocks, ...; seen on Geyser-Spigot 2.12): the Java block of the same path, in its default state.
fn java_name_of_custom(name: &str) -> String {
    match name.strip_prefix("geyser_custom:") {
        Some(path) => format!("minecraft:{path}"),
        None => name.to_owned(),
    }
}

/// For a Bedrock state Geyser never sends (e.g. `update_bit=1` leaves): the mapped state of the same
/// block sharing the most properties.
fn nearest_mapped(
    registry: &BlockRegistry,
    by_bedrock: &HashMap<(&str, &str), JavaState>,
    name: &str,
    props: &str,
) -> Option<JavaState> {
    let want: Vec<&str> = props.split(',').collect();
    registry
        .states_of(name)
        .filter_map(|(_, s)| {
            let java = by_bedrock.get(&(s.name, s.properties))?;
            let shared = s
                .properties
                .split(',')
                .filter(|kv| want.contains(kv))
                .count();
            Some((shared, std::cmp::Reverse(*java), *java))
        })
        .max()
        .map(|(_, _, java)| java)
}

/// Java biome names by Bedrock biome id; ids Geyser does not map read as plains.
pub struct Biomes {
    names: Vec<&'static str>,
    by_bedrock: HashMap<u32, u16>,
    plains: u16,
}

pub fn biomes() -> &'static Biomes {
    static B: OnceLock<Biomes> = OnceLock::new();
    B.get_or_init(|| {
        let list = &java().t.biomes;
        let names: Vec<&'static str> = list.iter().map(|&(_, n)| n).collect();
        let by_bedrock = list
            .iter()
            .enumerate()
            .map(|(i, &(id, _))| (id, i as u16))
            .collect();
        let plains = names
            .iter()
            .position(|n| *n == "minecraft:plains")
            .expect("plains") as u16;
        Biomes {
            names,
            by_bedrock,
            plains,
        }
    })
}

impl Biomes {
    pub fn java(&self, bedrock_id: u32) -> u16 {
        self.by_bedrock
            .get(&bedrock_id)
            .copied()
            .unwrap_or(self.plains)
    }

    pub fn name(&self, index: u16) -> &'static str {
        self.names[index as usize]
    }
}

#[cfg(test)]
mod tests;
