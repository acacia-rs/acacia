//! Java's tint colours per biome (`Biome.getGrassColor` and its kin), written under Bedrock's biome
//! names as the `biomes_client.json` the renderer reads from a look pack's files.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::download::Error;

/// Java biomes Bedrock calls something else, or splits into several.
const BEDROCK_NAMES: &[(&str, &[&str])] = &[
    ("swamp", &["swampland", "swampland_mutated"]),
    ("snowy_taiga", &["cold_taiga", "cold_taiga_hills", "cold_taiga_mutated"]),
    ("snowy_beach", &["cold_beach"]),
    ("snowy_plains", &["ice_plains", "ice_mountains"]),
    ("ice_spikes", &["ice_plains_spikes"]),
    ("frozen_ocean", &["frozen_ocean", "legacy_frozen_ocean"]),
    ("warm_ocean", &["warm_ocean", "deep_warm_ocean"]),
    ("dark_forest", &["roofed_forest", "roofed_forest_mutated"]),
    ("windswept_hills", &["extreme_hills", "extreme_hills_edge"]),
    ("windswept_gravelly_hills", &["extreme_hills_mutated", "extreme_hills_plus_trees_mutated"]),
    ("windswept_forest", &["extreme_hills_plus_trees"]),
    ("windswept_savanna", &["savanna_mutated", "savanna_plateau_mutated"]),
    ("old_growth_pine_taiga", &["mega_taiga", "mega_taiga_hills"]),
    ("old_growth_spruce_taiga", &["redwood_taiga_mutated", "redwood_taiga_hills_mutated"]),
    ("old_growth_birch_forest", &["birch_forest_mutated", "birch_forest_hills_mutated"]),
    ("nether_wastes", &["hell"]),
    ("soul_sand_valley", &["soulsand_valley"]),
    ("stony_shore", &["stone_beach"]),
    ("badlands", &["mesa", "mesa_plateau", "mesa_plateau_mutated"]),
    ("wooded_badlands", &["mesa_plateau_stone", "mesa_plateau_stone_mutated"]),
    ("eroded_badlands", &["mesa_bryce"]),
    ("sparse_jungle", &["jungle_edge", "jungle_edge_mutated"]),
    ("mushroom_fields", &["mushroom_island", "mushroom_island_shore"]),
    ("forest", &["forest", "forest_hills"]),
    ("birch_forest", &["birch_forest", "birch_forest_hills"]),
    ("taiga", &["taiga", "taiga_hills", "taiga_mutated"]),
    ("jungle", &["jungle", "jungle_hills", "jungle_mutated"]),
    ("desert", &["desert", "desert_hills", "desert_mutated"]),
    ("bamboo_jungle", &["bamboo_jungle", "bamboo_jungle_hills"]),
];
/// Java's water where a biome sets nothing else, for the biomes only Bedrock has.
const DEFAULT_WATER: u32 = 0x3F76E4;
/// `GrassColorModifier.SWAMP`: grass, and grass where the noise is low.
const SWAMP: (u32, u32) = (0x6A7039, 0x4C763C);

/// The `minecraft:visual/water_fog_*` attributes where a biome sets none (`EnvironmentAttributes`).
const DEFAULT_WATER_FOG: u32 = 0x050533;
const WATER_FOG_START: f32 = -8.0;
const WATER_FOG_END: f32 = 96.0;

/// A biome's colours as `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub water: u32,
    pub grass: u32,
    /// Grass where `acacia_render::biome::noise::grass_patch` says.
    pub grass_patch: Option<u32>,
    pub foliage: u32,
    pub dry_foliage: u32,
    /// Colour and opaque distance of the fog with the camera in water, at full water vision.
    pub water_fog: u32,
    pub water_fog_end: f32,
    /// The air's fog where the biome sets one (`minecraft:visual/fog_color`: the Nether's biomes).
    pub fog: Option<u32>,
}

/// The jar's 256×256 colormaps, row by row.
pub struct Colormaps {
    grass: Vec<u32>,
    foliage: Vec<u32>,
    dry_foliage: Vec<u32>,
}

impl Colormaps {
    /// `assets` is the jar's `assets/minecraft`.
    pub fn load(assets: &Path) -> Result<Colormaps, Error> {
        let map = |name: &str| -> Result<Vec<u32>, Error> {
            let image = image::open(assets.join(format!("textures/colormap/{name}.png"))).map_err(|e| e.to_string())?.into_rgb8();
            Ok(image.pixels().map(|p| u32::from(p.0[0]) << 16 | u32::from(p.0[1]) << 8 | u32::from(p.0[2])).collect())
        };
        Ok(Colormaps { grass: map("grass")?, foliage: map("foliage")?, dry_foliage: map("dry_foliage")? })
    }
}

/// `ColorMapColorUtil.get`.
fn sample(map: &[u32], temperature: f32, downfall: f32) -> u32 {
    let temperature = f64::from(temperature.clamp(0.0, 1.0));
    let rain = f64::from(downfall.clamp(0.0, 1.0)) * temperature;
    let (x, y) = (((1.0 - temperature) * 255.0) as usize, ((1.0 - rain) * 255.0) as usize);
    map.get(y << 8 | x).copied().unwrap_or(0)
}

/// Of one of the jar's `data/minecraft/worldgen/biome` files.
pub fn colors(biome: &Value, maps: &Colormaps) -> Option<Colors> {
    let effects = &biome["effects"];
    let color = |key: &str| u32::from_str_radix(effects[key].as_str()?.strip_prefix('#')?, 16).ok();
    let (temperature, downfall) = (biome["temperature"].as_f64()? as f32, biome["downfall"].as_f64()? as f32);
    let mapped = |map: &[u32]| sample(map, temperature, downfall);
    let base = color("grass_color").unwrap_or_else(|| mapped(&maps.grass));
    let (grass, grass_patch) = match effects["grass_color_modifier"].as_str() {
        Some("dark_forest") => (((base & 0xFEFEFE) + 0x28340A) >> 1, None),
        Some("swamp") => (SWAMP.0, Some(SWAMP.1)),
        _ => (base, None),
    };
    let attributes = &biome["attributes"];
    let hex_of = |key: &str| attributes[key].as_str().and_then(|c| u32::from_str_radix(c.strip_prefix('#')?, 16).ok());
    let water_fog = hex_of("minecraft:visual/water_fog_color");
    Some(Colors {
        fog: hex_of("minecraft:visual/fog_color"),
        water: color("water_color")?,
        grass,
        grass_patch,
        foliage: color("foliage_color").unwrap_or_else(|| mapped(&maps.foliage)),
        dry_foliage: color("dry_foliage_color").unwrap_or_else(|| mapped(&maps.dry_foliage)),
        water_fog: water_fog.unwrap_or(DEFAULT_WATER_FOG),
        water_fog_end: modified(&attributes["minecraft:visual/water_fog_end_distance"], WATER_FOG_END),
    })
}

/// An attribute's value over `base`: a number replaces it, `{argument, modifier: "multiply"}` scales it.
fn modified(attribute: &Value, base: f32) -> f32 {
    match (attribute.as_f64(), attribute["modifier"].as_str(), attribute["argument"].as_f64()) {
        (Some(v), ..) => v as f32,
        (None, Some("multiply"), Some(by)) => base * by as f32,
        _ => base,
    }
}

/// By Java name, from the jar's `data/minecraft/worldgen/biome`.
pub fn java(biomes: &Path, maps: &Colormaps) -> Result<BTreeMap<String, Colors>, Error> {
    let mut by_name = BTreeMap::new();
    for entry in std::fs::read_dir(biomes)? {
        let path = entry?.path();
        let (Some(name), Some("json")) = (path.file_stem().and_then(|n| n.to_str()), path.extension().and_then(|e| e.to_str())) else { continue };
        if let Some(colors) = colors(&serde_json::from_slice(&std::fs::read(&path)?)?, maps) {
            by_name.insert(name.to_owned(), colors);
        }
    }
    Ok(by_name)
}

pub fn write(java: &BTreeMap<String, Colors>, look: &Path) -> Result<(), Error> {
    let hex = |color: u32| format!("#{color:06x}");
    let mut by_name = Map::new();
    let default = json!({
        "water_surface_color": hex(DEFAULT_WATER),
        "water_fog_color": hex(DEFAULT_WATER_FOG),
        "water_fog_distance": [WATER_FOG_START, WATER_FOG_END],
    });
    by_name.insert("default".to_owned(), default);
    for (name, c) in java {
        let mut entry = json!({
            "water_surface_color": hex(c.water),
            "grass_color": hex(c.grass),
            "foliage_color": hex(c.foliage),
            "dry_foliage_color": hex(c.dry_foliage),
            "water_fog_color": hex(c.water_fog),
            "water_fog_distance": [WATER_FOG_START, c.water_fog_end],
        });
        if let Some(patch) = c.grass_patch {
            entry["grass_patch_color"] = hex(patch).into();
        }
        if let Some(fog) = c.fog {
            entry["fog_color"] = hex(fog).into();
        }
        for bedrock in bedrock_names(name) {
            by_name.insert(format!("minecraft:{bedrock}"), entry.clone());
        }
    }
    std::fs::write(look.join("biomes_client.json"), serde_json::to_vec_pretty(&json!({ "biomes": by_name }))?)?;
    Ok(())
}

/// What Bedrock calls a Java biome.
pub fn bedrock_names(java: &str) -> Vec<&str> {
    BEDROCK_NAMES.iter().find(|(name, _)| *name == java).map_or_else(|| vec![java], |(_, names)| names.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maps() -> Colormaps {
        // Each pixel its own index, so a sample says where it was taken.
        let map: Vec<u32> = (0..65536).collect();
        Colormaps { grass: map.clone(), foliage: map.clone(), dry_foliage: map }
    }

    #[test]
    fn a_biome_takes_its_overrides_then_the_colormaps() {
        let plains = json!({"temperature": 0.8, "downfall": 0.4, "effects": {"water_color": "#3f76e4"}});
        let at = (((1.0 - 0.8f32 as f64 * 0.4f32 as f64) * 255.0) as u32) << 8 | ((1.0 - 0.8f32 as f64) * 255.0) as u32;
        let water_fog = (DEFAULT_WATER_FOG, WATER_FOG_END);
        let plains_colors = Colors { water: 0x3F76E4, grass: at, grass_patch: None, foliage: at, dry_foliage: at, water_fog: water_fog.0, water_fog_end: water_fog.1, fog: None };
        assert_eq!(colors(&plains, &maps()), Some(plains_colors));
        let swamp = json!({"temperature": 0.8, "downfall": 0.9, "effects": {"water_color": "#617b64", "foliage_color": "#6a7039", "grass_color_modifier": "swamp"},
            "attributes": {"minecraft:visual/water_fog_color": "#232317", "minecraft:visual/water_fog_end_distance": {"argument": 0.85, "modifier": "multiply"}}});
        let swamp = colors(&swamp, &maps()).unwrap();
        assert_eq!((swamp.grass, swamp.grass_patch, swamp.foliage), (0x6A7039, Some(0x4C763C), 0x6A7039));
        assert_eq!((swamp.water_fog, swamp.water_fog_end), (0x232317, 96.0 * 0.85));
        let dark = json!({"temperature": 2.0, "downfall": 0.0, "effects": {"water_color": "#000000", "grass_color": "#fefefe", "grass_color_modifier": "dark_forest"}});
        assert_eq!(colors(&dark, &maps()).unwrap().grass, (0xFEFEFE + 0x28340A) >> 1);
        assert_eq!(colors(&json!({"temperature": 0.5, "downfall": 0.5, "effects": {}}), &maps()), None);
    }

    #[test]
    fn colours_land_under_bedrock_names() {
        let dir = std::env::temp_dir().join(format!("lookbake-biomes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let swamp = Colors { water: 0x617B64, grass: 0x6A7039, grass_patch: Some(0x4C763C), foliage: 1, dry_foliage: 2, water_fog: 0x232317, water_fog_end: 81.0, fog: Some(0x330808) };
        write(&BTreeMap::from([("swamp".to_owned(), swamp)]), &dir).unwrap();
        let written: Value = serde_json::from_slice(&std::fs::read(dir.join("biomes_client.json")).unwrap()).unwrap();
        let of = |name: &str, key: &str| written["biomes"][name][key].as_str().map(str::to_owned);
        assert_eq!(of("minecraft:swampland", "water_surface_color"), Some("#617b64".to_owned()));
        assert_eq!(of("minecraft:swampland_mutated", "grass_patch_color"), Some("#4c763c".to_owned()));
        assert_eq!(of("minecraft:swampland", "dry_foliage_color"), Some("#000002".to_owned()));
        assert_eq!(of("minecraft:swampland", "water_fog_color"), Some("#232317".to_owned()));
        assert_eq!(of("minecraft:swampland", "fog_color"), Some("#330808".to_owned()));
        assert_eq!(written["biomes"]["minecraft:swampland"]["water_fog_distance"], json!([-8.0, 81.0]));
        assert_eq!((of("default", "water_surface_color"), of("minecraft:swamp", "grass_color")), (Some("#3f76e4".to_owned()), None));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
