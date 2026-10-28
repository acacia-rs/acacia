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
pub(super) fn print(ours: &BTreeMap<String, Colors>, dump: &Path, outcome: &mut super::Outcome) -> Result<(), Error> {
    let dump: Dump = serde_json::from_slice(&std::fs::read(dump)?)?;
    let hex = |color: u32| format!("{color:06x}");
    // Each differing colour as (field, the game's, ours).
    let differences = |name: &String, theirs: &Biome| -> Vec<(String, String, String)> {
        let Some(c) = ours.get(name.trim_start_matches("minecraft:")) else { return vec![("biome".into(), name.clone(), "none".into())] };
        let grass = |&[x, z]: &[i32; 2]| hex(c.grass_patch.filter(|_| noise::grass_patch(x, z)).unwrap_or(c.grass));
        let fields = [("foliage", &theirs.foliage, hex(c.foliage)), ("dry_foliage", &theirs.dry_foliage, hex(c.dry_foliage)), ("water", &theirs.water, hex(c.water))];
        let columns = dump.columns.iter().zip(&theirs.grass).map(|(column, theirs)| (format!("grass at {column:?}"), theirs, grass(column)));
        let all = fields.into_iter().map(|(f, t, o)| (f.to_owned(), t, o)).chain(columns);
        all.filter(|(_, theirs, ours)| *theirs != ours).map(|(f, t, o)| (f, t.clone(), o)).collect()
    };
    let mut differing: Vec<&str> = Vec::new();
    for (name, theirs) in &dump.biomes {
        let found = differences(name, theirs);
        if !found.is_empty() {
            differing.push(name);
        }
        found.into_iter().for_each(|(field, theirs, ours)| outcome.diverge(name, &field, theirs, ours));
    }
    outcome.cases += dump.biomes.len();
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
