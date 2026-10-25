//! The Java models a look carries (`java/entity_models.json`, written by `lookbake java`), baked
//! for the kinds in [`super::kinds`]; [`crate::entity::EntityModels`] asks it first.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::kinds::{self, Look};
use super::{Family, Poses, anim, bedrock, mesh};
use crate::entity::bake::Mesh;
use crate::entity::molang::Value;
use crate::entity::{Layer, ModelId, Pose, TextureId};

pub const MODELS: &str = "java/entity_models.json";

#[derive(Default)]
pub struct Library {
    /// By Bedrock identifier: the adult's and the baby's (model, texture), and whether the baby
    /// has a model of its own.
    kinds: HashMap<&'static str, ([(ModelId, TextureId); 2], bool)>,
    /// The `setupAnim` of each baked layer.
    families: HashMap<ModelId, Family>,
}

impl Library {
    /// `model` takes a mesh and gives its id; `texture` registers an image path (under the
    /// look's root) and gives its id, `None` when the file is missing.
    pub fn load(root: &Path, model: &mut dyn FnMut(Mesh) -> ModelId, texture: &mut dyn FnMut(&str) -> Option<TextureId>) -> Library {
        let Ok(text) = std::fs::read_to_string(root.join(MODELS)) else { return Library::default() };
        let layers = match super::model::parse_layers(&text) {
            Ok(layers) => layers,
            Err(e) => {
                tracing::warn!("{MODELS}: {e}");
                return Library::default();
            }
        };
        let mut out = Library::default();
        let mut baked: HashMap<&str, ModelId> = HashMap::new();
        let mut variant = |look: &Look, families: &mut HashMap<ModelId, Family>| -> Option<(ModelId, TextureId)> {
            let id = match baked.get(look.layer) {
                Some(&id) => id,
                None => {
                    let id = model(mesh::bake(&Arc::new(layers.get(look.layer)?.clone())));
                    baked.insert(look.layer, id);
                    families.extend(Family::of_layer(look.layer).map(|f| (id, f)));
                    id
                }
            };
            Some((id, texture(&format!("java/textures/entity/{}", look.texture))?))
        };
        for (identifier, kind) in &kinds::KINDS {
            let Some(adult) = variant(&kind.adult, &mut out.families) else { continue };
            let baby = kind.baby.as_ref().and_then(|b| variant(b, &mut out.families));
            out.kinds.insert(identifier, ([adult, baby.unwrap_or(adult)], baby.is_some()));
        }
        tracing::info!(kinds = out.kinds.len(), layers = layers.len(), "java entity models");
        out
    }

    /// The layer and scale for `kind`, when it draws a Java model. A baby with its own model is
    /// drawn at full size, undoing the 0.5 the server sends (README "Java entity models").
    pub fn appearance(&self, kind: &str, query: &dyn Fn(&str) -> Value) -> Option<(Arc<[Layer]>, f32)> {
        let (variants, own_baby) = self.kinds.get(kind)?;
        let baby = query("is_baby").truthy();
        let (model, texture) = variants[usize::from(baby)];
        let scale = if baby && *own_baby { 2.0 } else { 1.0 };
        Some(([Layer::plain(model, texture)].into(), scale))
    }

    /// The pose of a Java model (`mesh`, model `id`); `None` for a Bedrock one.
    pub fn pose(&self, id: ModelId, mesh: &Mesh, query: &dyn Fn(&str) -> Value) -> Option<Pose> {
        let model = mesh.java.as_ref()?;
        let mut poses = Poses::new(model);
        if let Some(&family) = self.families.get(&id) {
            anim::setup(family, &bedrock::render_state(family, query), &mut poses);
        }
        Some(Pose::java(poses))
    }
}
