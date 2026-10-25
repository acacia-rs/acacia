//! Armour trims, as `EquipmentLayerRenderer` draws them over a piece's layers (not on babies):
//! the pattern's texture for the layer type in the material's palette, unless the equipment
//! asset's first matching `trim_overrides` entry swaps the texture or palette (gold trims gold
//! armour in `trim/gold_darker`); an override without a palette draws the grey texture as it is.
//! The paletted textures are baked by lookbake (`trims.rs`) to [`baked_path`].

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

const DIR: &str = "java/textures/trims";

/// Where lookbake bakes a pattern asset's texture for `layer_type` in `palette` (both ids).
pub fn baked_path(layer_type: &str, asset: &str, palette: &str) -> String {
    let name = |id: &str| id.strip_prefix("minecraft:").unwrap_or(id).replace('/', "_");
    format!("{DIR}/baked/{layer_type}/{}/{}.png", name(asset), name(palette))
}

fn raw_path(layer_type: &str, asset: &str) -> String {
    format!("{DIR}/entity/{layer_type}/{}.png", asset.strip_prefix("minecraft:").unwrap_or(asset))
}

/// A stack's trim, by Bedrock's names (`Material`, `Pattern`): `iron`, `coast`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trim {
    pub material: String,
    pub pattern: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct When {
    material: Option<String>,
    pattern: Option<String>,
}

/// `EquipmentClientInfo.TrimOverride`.
#[derive(Deserialize, Clone)]
pub struct Override {
    when: When,
    texture: Option<String>,
    palette: Option<String>,
}

#[derive(Default)]
pub struct Trims {
    /// Material to its palette id (`minecraft:trim/iron`).
    palettes: HashMap<String, String>,
    /// Pattern to its asset id (`minecraft:coast`).
    assets: HashMap<String, String>,
}

impl Trims {
    pub fn load(root: &Path) -> Trims {
        let read = |dir: &str, field: &str| -> HashMap<String, String> {
            let Ok(entries) = std::fs::read_dir(root.join(dir)) else { return HashMap::new() };
            let pairs = entries.flatten().filter_map(|e| {
                let json: serde_json::Value = serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok()?;
                Some((format!("minecraft:{}", e.path().file_stem()?.to_str()?), json[field].as_str()?.to_owned()))
            });
            pairs.collect()
        };
        Trims { palettes: read("java/trims/material", "palette_id"), assets: read("java/trims/pattern", "asset_id") }
    }

    /// The image `trim` draws for `layer_type` on a piece whose asset has `overrides`; `None` for
    /// an unknown material or pattern.
    pub fn texture(&self, overrides: &[Override], layer_type: &str, trim: &Trim) -> Option<String> {
        let id = |name: &str| if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") };
        let (material, pattern) = (id(&trim.material), id(&trim.pattern));
        let mut asset = self.assets.get(&pattern)?.clone();
        let mut palette = Some(self.palettes.get(&material)?.clone());
        let matches = |o: &&Override| o.when.material.as_ref().is_none_or(|m| *m == material) && o.when.pattern.as_ref().is_none_or(|p| *p == pattern);
        if let Some(o) = overrides.iter().find(matches) {
            asset = o.texture.clone().unwrap_or(asset);
            palette = o.palette.clone();
        }
        Some(match palette {
            Some(palette) => baked_path(layer_type, &asset, &palette),
            None => raw_path(layer_type, &asset),
        })
    }

    /// Every trim image in the look, for registering up front.
    pub fn images(root: &Path) -> Vec<String> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else if path.extension().is_some_and(|e| e == "png") {
                    out.extend(path.strip_prefix(root).ok().and_then(|p| p.to_str()).map(|p| p.replace('\\', "/")));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, &root.join(DIR), &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_material_override_swaps_the_palette() {
        let trims = Trims {
            palettes: HashMap::from([("minecraft:gold".into(), "minecraft:trim/gold".into()), ("minecraft:iron".into(), "minecraft:trim/iron".into())]),
            assets: HashMap::from([("minecraft:coast".into(), "minecraft:coast".into())]),
        };
        let gold: Vec<Override> = serde_json::from_str(r#"[{"palette": "minecraft:trim/gold_darker", "when": {"material": "minecraft:gold"}}]"#).unwrap();
        let trim = |material: &str| Trim { material: material.into(), pattern: "coast".into() };
        assert_eq!(trims.texture(&gold, "humanoid", &trim("gold")).unwrap(), "java/textures/trims/baked/humanoid/coast/trim_gold_darker.png");
        assert_eq!(trims.texture(&gold, "humanoid", &trim("iron")).unwrap(), "java/textures/trims/baked/humanoid/coast/trim_iron.png");
        let plain: Vec<Override> = serde_json::from_str(r#"[{"when": {"pattern": "minecraft:coast"}}]"#).unwrap();
        assert_eq!(trims.texture(&plain, "humanoid_leggings", &trim("iron")).unwrap(), "java/textures/trims/entity/humanoid_leggings/coast.png");
        assert!(trims.texture(&[], "humanoid", &trim("wood")).is_none());
    }
}
