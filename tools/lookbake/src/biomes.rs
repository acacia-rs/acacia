//! Java's water colour per biome, written as the `biomes_client.json` the renderer reads from a
//! look pack's files, under Bedrock's biome names. Grass and foliage need nothing: the renderer's
//! colormap sampling and per-biome exceptions are Java's already.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::download::Error;

/// Java biomes Bedrock calls something else, or splits into several.
const BEDROCK_NAMES: &[(&str, &[&str])] = &[
    ("swamp", &["swampland", "swampland_mutated"]),
    ("snowy_taiga", &["cold_taiga", "cold_taiga_hills", "cold_taiga_mutated"]),
    ("snowy_beach", &["cold_beach"]),
    ("frozen_ocean", &["frozen_ocean", "legacy_frozen_ocean"]),
    ("warm_ocean", &["warm_ocean", "deep_warm_ocean"]),
];
/// Java's water where a biome sets nothing else, for the biomes only Bedrock has.
const DEFAULT_WATER: &str = "#3f76e4";

/// `biomes` is the jar's `data/minecraft/worldgen/biome`.
pub fn write(biomes: &Path, look: &Path) -> Result<(), Error> {
    let mut by_name = Map::new();
    by_name.insert("default".to_owned(), json!({ "water_surface_color": DEFAULT_WATER }));
    for entry in std::fs::read_dir(biomes)? {
        let path = entry?.path();
        let (Some(name), Some("json")) = (path.file_stem().and_then(|n| n.to_str()), path.extension().and_then(|e| e.to_str())) else { continue };
        let biome: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        let Some(water) = biome["effects"]["water_color"].as_str() else { continue };
        for bedrock in bedrock_names(name) {
            by_name.insert(format!("minecraft:{bedrock}"), json!({ "water_surface_color": water }));
        }
    }
    std::fs::write(look.join("biomes_client.json"), serde_json::to_vec_pretty(&json!({ "biomes": by_name }))?)?;
    Ok(())
}

fn bedrock_names(java: &str) -> Vec<&str> {
    BEDROCK_NAMES.iter().find(|(name, _)| *name == java).map_or_else(|| vec![java], |(_, names)| names.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_colours_land_under_bedrock_names() {
        let dir = std::env::temp_dir().join(format!("lookbake-biomes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("biome")).unwrap();
        std::fs::write(dir.join("biome/swamp.json"), r##"{"effects": {"water_color": "#617b64"}}"##).unwrap();
        std::fs::write(dir.join("biome/the_void.json"), r##"{"effects": {}}"##).unwrap();

        write(&dir.join("biome"), &dir).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(dir.join("biomes_client.json")).unwrap()).unwrap();
        let water = |name: &str| written["biomes"][name]["water_surface_color"].as_str().map(str::to_owned);
        assert_eq!(water("minecraft:swampland"), Some("#617b64".to_owned()));
        assert_eq!(water("minecraft:swampland_mutated"), Some("#617b64".to_owned()));
        assert_eq!((water("default"), water("minecraft:swamp"), water("minecraft:the_void")), (Some(DEFAULT_WATER.to_owned()), None, None));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
