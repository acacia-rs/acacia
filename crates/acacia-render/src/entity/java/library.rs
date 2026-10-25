//! The Java models a look carries (`java/entity_models.json`, written by `lookbake java`), baked
//! for the kinds in [`super::kinds`]; [`crate::entity::EntityModels`] asks it first.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::kinds::{self, Kind, Look};
use super::{Poses, anim, bedrock, mesh};
use crate::entity::bake::Mesh;
use crate::entity::molang::Value;
use crate::entity::{Layer, ModelId, Pose, TextureId};

pub const MODELS: &str = "java/entity_models.json";

#[derive(Default)]
pub struct Library {
    /// By Bedrock identifier: adult and baby (model, texture).
    kinds: HashMap<&'static str, ([(ModelId, TextureId); 2], &'static Kind)>,
}

impl Library {
    /// `model` takes a mesh and gives its id; `texture` registers an image path (under `root`)
    /// and gives its id, `None` when the file is missing.
    pub fn load(root: &Path, model: &mut dyn FnMut(Mesh) -> ModelId, texture: &mut dyn FnMut(&str) -> Option<TextureId>) -> Library {
        let Ok(text) = std::fs::read_to_string(root.join(MODELS)) else { return Library::default() };
        let layers = match super::model::parse_layers(&text) {
            Ok(layers) => layers,
            Err(e) => {
                tracing::warn!("{MODELS}: {e}");
                return Library::default();
            }
        };
        let mut baked: HashMap<&str, ModelId> = HashMap::new();
        let mut variant = |look: &Look| -> Option<(ModelId, TextureId)> {
            let id = match baked.get(look.layer) {
                Some(&id) => id,
                None => {
                    let id = model(mesh::bake(&Arc::new(layers.get(look.layer)?.clone())));
                    baked.insert(look.layer, id);
                    id
                }
            };
            Some((id, texture(&format!("java/textures/entity/{}", look.texture))?))
        };
        let mut out = Library::default();
        for (identifier, kind) in &kinds::KINDS {
            let Some(adult) = variant(&kind.adult) else { continue };
            let baby = kind.baby.as_ref().and_then(&mut variant).unwrap_or(adult);
            out.kinds.insert(identifier, ([adult, baby], kind));
        }
        tracing::info!(kinds = out.kinds.len(), layers = layers.len(), "java entity models");
        out
    }

    /// The layer and scale for `kind`, when it draws a Java model. A baby with its own model is
    /// drawn at full size: the 0.5 the server sends is undone (an assumption the entity_layers
    /// harness checks).
    pub fn appearance(&self, kind: &str, query: &dyn Fn(&str) -> Value) -> Option<(Arc<[Layer]>, f32)> {
        let (variants, java) = self.kinds.get(kind)?;
        let baby = query("is_baby").truthy();
        let (model, texture) = variants[usize::from(baby)];
        let scale = if baby && java.baby.is_some() { 2.0 } else { 1.0 };
        Some(([Layer::plain(model, texture)].into(), scale))
    }

    /// The pose of a Java model, `None` for a Bedrock one.
    pub fn pose(&self, mesh: &Mesh, kind: &str, query: &dyn Fn(&str) -> Value) -> Option<Pose> {
        let model = mesh.java.as_ref()?;
        let mut poses = Poses::new(model);
        if let Some((_, java)) = self.kinds.get(kind) {
            anim::setup(java.family, &bedrock::render_state(java.family, query), &mut poses);
        }
        Some(Pose::java(poses))
    }
}
