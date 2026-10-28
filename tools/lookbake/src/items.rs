//! Java's item models, as the game resolves them: the exporter `ItemModelExporter` (exporter.rs)
//! writes every item's default stack in every display context, which goes into the look as
//! `java/item_models.json` with the images its quads sample (under `java/textures/`), and
//! `java/item_names.json`, which Java item a Bedrock item and aux is (ViaBedrock's table, GPL-3.0,
//! downloaded when baking like the block states'; see mapping.rs).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::download::{Error, get, pinned};
use crate::exporter::Exporter;

pub const MODELS: &str = "java/item_models.json";
pub const NAMES: &str = "java/item_names.json";

const COMMIT: &str = "6e242b278740ceccac20e5db0915b00775095b24";
const FILE: &str = "src/main/resources/assets/viabedrock/data/custom/item_mappings.json";
const SHA1: &str = "b6e8ccd13cc50b4327e6d9a5129d8a094276e0ed";

pub struct Baked {
    pub items: usize,
    pub images: usize,
    pub names: usize,
}

/// `assets` is the jar's `assets/minecraft`; `java_dir` holds the downloads.
pub fn bake(exporter: &Exporter, java_dir: &Path, assets: &Path, out: &Path) -> Result<Baked, Error> {
    let models = exporter.output("ItemModelExporter")?;
    let json: Value = serde_json::from_slice(&std::fs::read(&models)?)?;
    let images: BTreeSet<&str> = json["meshes"].as_array().into_iter().flatten()
        .flat_map(|mesh| mesh["quads"].as_array().into_iter().flatten())
        .filter_map(|quad| quad["image"].as_str())
        .collect();
    for image in &images {
        let path = image.strip_prefix("minecraft:").ok_or_else(|| format!("image outside minecraft: {image}"))?;
        for file in [path.to_owned(), format!("{path}.mcmeta")] {
            let (from, to) = (assets.join(&file), out.join("java").join(&file));
            if from.is_file() {
                std::fs::create_dir_all(to.parent().expect("a file in a directory"))?;
                std::fs::copy(from, to)?;
            }
        }
    }
    let to = out.join(MODELS);
    std::fs::create_dir_all(to.parent().expect("a file in a directory"))?;
    std::fs::copy(&models, to)?;
    let url = format!("https://raw.githubusercontent.com/RaphiMC/ViaBedrock/{COMMIT}/{FILE}");
    let names = names(&serde_json::from_slice(&pinned(&java_dir.join("item_mappings.json"), SHA1, || get(&url))?)?);
    std::fs::write(out.join(NAMES), serde_json::to_vec(&names)?)?;
    Ok(Baked { items: json["items"].as_object().map_or(0, |i| i.len()), images: images.len(), names: names.len() })
}

/// By Bedrock item, the Java item per aux (`""` for any). Items keyed by block state map to one
/// Java item each in the table.
fn names(table: &Value) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (bedrock, entry) in table.as_object().into_iter().flatten() {
        let mut by_aux = BTreeMap::new();
        for (aux, java) in entry["meta"].as_object().into_iter().flatten() {
            if let Some(id) = java["java_id"].as_str() {
                by_aux.insert(aux.clone(), id.to_owned());
            }
        }
        if let Some(id) = entry["block"].as_object().and_then(|b| b.values().next()).and_then(|java| java["java_id"].as_str()) {
            by_aux.insert(String::new(), id.to_owned());
        }
        if !by_aux.is_empty() {
            out.insert(bedrock.clone(), by_aux);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_by_aux_and_by_block() {
        let table = serde_json::json!({
            "minecraft:wooden_door": {"meta": {"": {"java_id": "minecraft:oak_door"}}},
            "minecraft:potion": {"meta": {"": {"java_id": "minecraft:potion"}, "5": {"java_id": "minecraft:potion", "java_tag": "{}"}}},
            "minecraft:acacia_log": {"block": {"minecraft:acacia_log[pillar_axis=y]": {"java_id": "minecraft:acacia_log"}}}
        });
        let names = names(&table);
        assert_eq!(names["minecraft:wooden_door"][""], "minecraft:oak_door");
        assert_eq!(names["minecraft:potion"].len(), 2);
        assert_eq!(names["minecraft:acacia_log"][""], "minecraft:acacia_log");
    }
}
