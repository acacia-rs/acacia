//! Entity models from the vanilla pack: client entity definitions (`entity/*.entity.json`) name a
//! geometry and a texture per entity kind; geometries bake to rest-pose meshes. See README "Entities".

pub mod bake;
pub mod geometry;
mod skin;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec3;
use serde_json::Value;

use crate::assets::{Pack, json};
pub use bake::{Mesh, Vertex};
pub use skin::{Skin, SkinSource};

pub type ModelId = u32;

pub struct Model {
    pub mesh: Mesh,
    /// Image files of the default look, bottom layer first; empty when the pack lacks them.
    pub textures: Vec<PathBuf>,
}

/// Texture keys of the default look, in layer order. Villagers split theirs into skin, biome
/// clothes and profession; everything else has one `default`.
const LAYERS: [&[&str]; 2] = [&["default"], &["base", "plains", "unskilled"]];

/// Size of kinds without a baby geometry when [`EntityModels::lookup`] is asked for a baby.
const BABY_SCALE: f32 = 0.5;
const PLAYER_GEOMETRIES: [&str; 3] = ["geometry.humanoid.custom", "geometry.humanoid.customSlim", "geometry.humanoid"];

#[derive(Default)]
pub struct EntityModels {
    models: Vec<Model>,
    /// Kind identifier (`minecraft:cow`) to its adult and baby models.
    kinds: HashMap<String, (ModelId, Option<ModelId>)>,
    /// Wide arms, slim arms, 64×32 skin layout.
    players: Option<[ModelId; 3]>,
}

/// One entity to draw this frame. Angles in degrees, as the server sends them.
#[derive(Clone)]
pub struct EntityInstance {
    pub model: ModelId,
    /// Replaces the model's texture.
    pub skin: Option<Arc<Skin>>,
    /// Feet.
    pub position: DVec3,
    pub yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub scale: f32,
}

impl EntityModels {
    pub fn load(pack: &Pack) -> EntityModels {
        let geometries = geometry::load_all(pack.root());
        let (mut models, mut kinds) = (Vec::new(), HashMap::new());
        let mut add = |geometry: &str, textures: &[String]| -> Option<ModelId> {
            let mesh = bake::bake(geometries.get(geometry)?);
            models.push(Model { mesh, textures: textures.iter().filter_map(|t| pack.image_file(t)).collect() });
            Some(models.len() as ModelId - 1)
        };
        let definitions = definitions(pack);
        for (kind, d) in &definitions {
            let pick = |map: &HashMap<String, String>, keys: &[&str]| -> Vec<String> { keys.iter().filter_map(|k| map.get(*k).cloned()).collect() };
            let Some(adult) = d.geometry.get("default").or_else(|| d.geometry.values().next()) else { continue };
            let mut textures = LAYERS.iter().map(|keys| pick(&d.textures, keys)).find(|t| !t.is_empty()).unwrap_or_default();
            if textures.is_empty() {
                textures.extend(d.textures.values().min().cloned());
            }
            let Some(adult) = add(adult, &textures) else { continue };
            let baby_textures = Some(pick(&d.textures, &["baby_default"])).filter(|t| !t.is_empty()).unwrap_or(textures);
            let baby = d.geometry.get("baby").and_then(|g| add(g, &baby_textures));
            kinds.insert(kind.clone(), (adult, baby));
        }
        let steve: Vec<String> = definitions.get("minecraft:player").and_then(|d| d.textures.get("default")).cloned().into_iter().collect();
        let players = PLAYER_GEOMETRIES.map(|g| add(g, &steve));
        let players = players.iter().all(Option::is_some).then(|| players.map(Option::unwrap_or_default));
        tracing::info!(kinds = kinds.len(), models = models.len(), "entity models");
        EntityModels { models, kinds, players }
    }

    pub fn models(&self) -> &[Model] {
        &self.models
    }

    /// The model for an entity kind and the scale to draw it at.
    pub fn lookup(&self, kind: &str, baby: bool) -> Option<(ModelId, f32)> {
        let &(adult, baby_model) = self.kinds.get(kind)?;
        Some(match (baby, baby_model) {
            (true, Some(model)) => (model, 1.0),
            (true, None) => (adult, BABY_SCALE),
            (false, _) => (adult, 1.0),
        })
    }

    /// The humanoid for a skin: slim or wide arms, or the old layout when the skin is half height.
    pub fn player(&self, skin: Option<(&Skin, bool)>) -> Option<ModelId> {
        let [wide, slim, legacy] = self.players?;
        Some(match skin {
            Some((s, _)) if s.height * 2 == s.width => legacy,
            Some((_, true)) => slim,
            _ => wide,
        })
    }
}

struct Definition {
    /// `(min_engine_version, format_version)`: the newest definition of a kind wins.
    version: (Vec<u32>, Vec<u32>),
    geometry: HashMap<String, String>,
    textures: HashMap<String, String>,
}

fn definitions(pack: &Pack) -> HashMap<String, Definition> {
    let mut out: HashMap<String, Definition> = HashMap::new();
    let files = std::fs::read_dir(pack.root().join("entity")).into_iter().flatten().flatten();
    for file in files.map(|e| e.path()) {
        let Ok(v) = json::read(&file) else { continue };
        let Some(d) = v.pointer("/minecraft:client_entity/description") else { continue };
        let Some(kind) = d.get("identifier").and_then(Value::as_str) else { continue };
        let version = |v: Option<&Value>| -> Vec<u32> {
            v.and_then(Value::as_str).map(|s| s.split('.').filter_map(|p| p.parse().ok()).collect()).unwrap_or_default()
        };
        let map = |key: &str| -> HashMap<String, String> {
            let entries = d.get(key).and_then(Value::as_object).into_iter().flatten();
            entries.filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned()))).collect()
        };
        let definition = Definition {
            version: (version(d.get("min_engine_version")), version(v.get("format_version"))),
            geometry: map("geometry"),
            textures: map("textures"),
        };
        if out.get(kind).is_none_or(|old| old.version < definition.version) {
            out.insert(kind.to_owned(), definition);
        }
    }
    out
}
