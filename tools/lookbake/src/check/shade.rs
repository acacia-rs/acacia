//! Which states darken ambient occlusion, against the dump's `shade.json`: the Java states the
//! game gives a shade brightness below 1.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use acacia_render::lookpack::state_key;
use acacia_world::BlockRegistry;
use serde::Deserialize;

use crate::download::Error;
use crate::mapping::{JavaState, Mapping};
use crate::shade;

#[derive(Deserialize)]
struct Dump {
    /// State keys as the quad dump spells them.
    dark: HashSet<String>,
}

/// `minecraft:oak_slab[type=double,waterlogged=false]`, properties sorted by name.
fn dump_key(java: &JavaState) -> String {
    let mut pairs: Vec<String> = java.properties.iter().map(|(k, v)| format!("{k}={v}")).collect();
    pairs.sort_unstable();
    if pairs.is_empty() { format!("minecraft:{}", java.name) } else { format!("minecraft:{}[{}]", java.name, pairs.join(",")) }
}

/// Every vanilla Bedrock state the mapping gives a Java state.
pub(super) fn print(mapping: &Mapping, dump: &Path) -> Result<(), Error> {
    let dump: Dump = serde_json::from_slice(&std::fs::read(dump)?)?;
    let registry = BlockRegistry::vanilla();
    let (mut same, mut differing): (usize, BTreeMap<&str, (usize, bool)>) = (0, BTreeMap::new());
    for state in (0..registry.len() as u32).filter_map(|id| registry.get(id)) {
        let Some(java) = mapping.get(&state_key(state)) else { continue };
        let (ours, theirs) = (shade::darkens(java, state.is_full_cube()), dump.dark.contains(&dump_key(java)));
        if ours == theirs {
            same += 1;
        } else {
            differing.entry(java.name.as_str()).or_insert((0, theirs)).0 += 1;
        }
    }
    let wrong: usize = differing.values().map(|(count, _)| count).sum();
    println!("{same} of {} mapped states darken ambient occlusion as the game has them", same + wrong);
    for dark in [true, false] {
        let names: Vec<String> = differing.iter().filter(|(_, (_, theirs))| *theirs == dark).map(|(name, (count, _))| format!("{name} ({count})")).collect();
        if !names.is_empty() {
            println!("  the game's {}: {}", if dark { "darken, ours do not" } else { "stay bright, ours darken" }, names.join(" "));
        }
    }
    Ok(())
}
