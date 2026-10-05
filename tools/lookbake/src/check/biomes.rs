//! Biome colours against the dump's `biomes.json`: what the game's `Biome` answers for foliage,
//! dry foliage and water, and for grass at sample columns.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use acacia_render::biome::{ids, noise};
use serde::Deserialize;

use crate::biomes::{self, Colors};
use crate::download::Error;

#[derive(Deserialize)]
struct Dump {
    columns: Vec<[i32; 2]>,
    /// By `minecraft:` name; colours as `rrggbb`.
    biomes: BTreeMap<String, Biome>,
}

#[derive(Deserialize)]
struct Biome {
    foliage: String,
    dry_foliage: String,
    water: String,
    /// At each of `columns`.
    grass: Vec<String>,
}

/// `ours` by Java name; `dump` is the game's `biomes.json`.
pub(super) fn print(ours: &BTreeMap<String, Colors>, dump: &Path) -> Result<(), Error> {
    let dump: Dump = serde_json::from_slice(&std::fs::read(dump)?)?;
    let hex = |color: u32| format!("{color:06x}");
    let same = |name: &String, theirs: &Biome| {
        let Some(c) = ours.get(name.trim_start_matches("minecraft:")) else { return false };
        let grass = |&[x, z]: &[i32; 2]| hex(c.grass_patch.filter(|_| noise::grass_patch(x, z)).unwrap_or(c.grass));
        hex(c.foliage) == theirs.foliage
            && hex(c.dry_foliage) == theirs.dry_foliage
            && hex(c.water) == theirs.water
            && dump.columns.iter().map(grass).eq(theirs.grass.iter().cloned())
    };
    let differing: Vec<&str> = dump.biomes.iter().filter(|(name, theirs)| !same(name, theirs)).map(|(name, _)| name.as_str()).collect();
    println!("{} of {} biomes colour as the game colours them", dump.biomes.len() - differing.len(), dump.biomes.len());
    if !differing.is_empty() {
        println!("  differing: {}", differing.join(" "));
    }
    let covered: BTreeSet<&str> = ours.keys().flat_map(|name| biomes::bedrock_names(name)).collect();
    let bare: Vec<&str> = ids::names().filter(|name| !covered.contains(name)).collect();
    if !bare.is_empty() {
        println!("  Bedrock biomes no Java biome colours: {}", bare.join(" "));
    }
    Ok(())
}
