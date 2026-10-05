// Ported from Pomme (https://github.com/PommeMC/Client), pomme-client/src/world/block/model.rs.
// Copyright (C) 2026 Purdze. GPL-3.0-or-later; see ../../LICENSE-pomme.

//! Java block model files (`models/<id>.json`): elements and texture slots, inherited from parents.

pub mod bake;
mod turn;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

/// Longest parent chain followed.
const MAX_PARENTS: usize = 20;
/// Longest chain of `#slot` references followed.
const MAX_REFERENCES: u32 = 10;

#[derive(Deserialize, Default, Clone)]
struct ModelFile {
    parent: Option<String>,
    #[serde(default, deserialize_with = "texture_map")]
    textures: HashMap<String, String>,
    #[serde(default)]
    elements: Vec<Element>,
    ambientocclusion: Option<bool>,
}

/// Slots hold a texture id or, in newer files, `{"sprite": id}`.
fn texture_map<'de, D: serde::Deserializer<'de>>(de: D) -> Result<HashMap<String, String>, D::Error> {
    let raw: HashMap<String, Value> = HashMap::deserialize(de)?;
    let id = |v: &Value| v.as_str().or_else(|| v.get("sprite")?.as_str()).map(str::to_owned);
    Ok(raw.into_iter().filter_map(|(slot, v)| Some((slot, id(&v)?))).collect())
}

#[derive(Deserialize, Clone)]
pub struct Element {
    pub from: [f32; 3],
    pub to: [f32; 3],
    #[serde(default)]
    pub rotation: Option<ElementRotation>,
    /// By face name; sorted, so a bake comes out the same every time.
    #[serde(default)]
    pub faces: BTreeMap<String, FaceDef>,
    /// Files from before 26.1.
    #[serde(default = "yes")]
    pub shade: bool,
    #[serde(default)]
    pub shade_direction_override: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, Clone)]
/// About one `axis` by `angle`, or (26.1 on) about `x`, `y` and `z`.
pub struct ElementRotation {
    pub origin: [f32; 3],
    pub axis: Option<String>,
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default)]
    pub rescale: bool,
}

#[derive(Deserialize, Clone)]
pub struct FaceDef {
    pub uv: Option<[f32; 4]>,
    pub texture: String,
    pub cullface: Option<String>,
    #[serde(default)]
    pub rotation: Option<i32>,
    pub tintindex: Option<i32>,
}

/// A model with its parents folded in.
#[derive(Default)]
pub struct Resolved {
    /// Slot to texture id (`block/oak_planks`), references followed.
    pub textures: HashMap<String, String>,
    pub elements: Vec<Element>,
    pub ambient_occlusion: bool,
}

impl Resolved {
    /// The texture id a face's `#slot` names.
    pub fn texture(&self, reference: &str) -> Option<&str> {
        let id = self.textures.get(reference.strip_prefix('#').unwrap_or(reference))?;
        (!id.starts_with('#')).then_some(id.as_str())
    }
}

pub struct Models {
    /// The jar's `assets/minecraft`.
    assets: PathBuf,
    files: HashMap<String, Option<ModelFile>>,
    /// Models whose JSON did not parse.
    pub invalid: Vec<String>,
}

pub fn strip_namespace(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

impl Models {
    pub fn new(assets: PathBuf) -> Models {
        Models { assets, files: HashMap::new(), invalid: Vec::new() }
    }

    fn file(&mut self, id: &str) -> Option<&ModelFile> {
        if !self.files.contains_key(id) {
            let text = std::fs::read(self.assets.join(format!("models/{id}.json"))).ok();
            let file = text.and_then(|t| serde_json::from_slice(&t).inspect_err(|_| self.invalid.push(id.to_owned())).ok());
            self.files.insert(id.to_owned(), file);
        }
        self.files[id].as_ref()
    }

    /// `id` without namespace: `block/oak_stairs`. The child's elements and slots win over its parents'.
    pub fn resolve(&mut self, id: &str) -> Resolved {
        let (mut slots, mut elements, mut ambient_occlusion) = (HashMap::new(), None, None);
        let mut current = id.to_owned();
        for _ in 0..MAX_PARENTS {
            let Some(file) = self.file(&current) else { break };
            for (slot, texture) in &file.textures {
                slots.entry(slot.clone()).or_insert_with(|| texture.clone());
            }
            if elements.is_none() && !file.elements.is_empty() {
                elements = Some(file.elements.clone());
            }
            ambient_occlusion = ambient_occlusion.or(file.ambientocclusion);
            match &file.parent {
                Some(parent) => current = strip_namespace(parent).to_owned(),
                None => break,
            }
        }
        let textures = slots.iter().map(|(slot, value)| (slot.clone(), strip_namespace(&follow(value, &slots, 0)).to_owned())).collect();
        Resolved { textures, elements: elements.unwrap_or_default(), ambient_occlusion: ambient_occlusion.unwrap_or(true) }
    }
}

fn follow(value: &str, slots: &HashMap<String, String>, depth: u32) -> String {
    match value.strip_prefix('#').and_then(|slot| slots.get(slot)) {
        Some(target) if depth <= MAX_REFERENCES => follow(target, slots, depth + 1),
        _ => value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_inherits_elements_and_fills_its_parents_slots() {
        let dir = std::env::temp_dir().join(format!("lookbake-models-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("models/block")).unwrap();
        let write = |name: &str, json: &str| std::fs::write(dir.join(format!("models/block/{name}.json")), json).unwrap();
        write("cube", r##"{"elements": [{"from": [0,0,0], "to": [16,16,16], "faces": {"up": {"texture": "#up", "cullface": "up"}}}]}"##);
        write("cube_all", r##"{"parent": "minecraft:block/cube", "ambientocclusion": false, "textures": {"up": "#all", "particle": "#all"}}"##);
        write("stone", r##"{"parent": "minecraft:block/cube_all", "textures": {"all": "minecraft:block/stone"}}"##);
        write("broken", "{");

        let mut models = Models::new(dir.clone());
        let stone = models.resolve("block/stone");
        assert_eq!(stone.elements.len(), 1);
        assert!(!stone.ambient_occlusion && models.resolve("block/cube").ambient_occlusion);
        assert_eq!((stone.texture("#up"), stone.texture("#missing")), (Some("block/stone"), None));
        assert!(models.resolve("block/broken").elements.is_empty() && models.resolve("block/absent").elements.is_empty());
        assert_eq!(models.invalid, ["block/broken"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
