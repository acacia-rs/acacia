//! The Java models a look carries (`java/entity_models.json`, written by `lookbake java`), baked
//! for the kinds in [`super::kinds`]; [`crate::entity::EntityModels`] asks it first.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::equipment::{self, Equipment};
use super::kinds::{self, Look};
use super::{Family, Poses, anim, bedrock, mesh};
use crate::entity::bake::Mesh;
use crate::entity::molang::Value;
use crate::entity::{Layer, ModelId, NO_TEXTURE, Pose, TextureId};

pub const MODELS: &str = "java/entity_models.json";

#[derive(Default)]
pub struct Library {
    /// By Bedrock identifier: the adult's and the baby's (model, texture), and whether the baby
    /// has a model of its own.
    kinds: HashMap<&'static str, ([(ModelId, TextureId); 2], bool)>,
    /// The `setupAnim` of each baked layer; an armour layer's is its wearer's.
    families: HashMap<ModelId, Family>,
    /// By Bedrock identifier, the adult's and the baby's armour models per slot ([`equipment::SLOTS`]).
    armor: HashMap<&'static str, [[Option<ModelId>; 4]; 2]>,
    equipment: Equipment,
    /// Equipment textures by path.
    equipment_textures: HashMap<String, TextureId>,
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
        for (identifier, kind) in kinds::KINDS.iter().filter(|(id, _)| out.kinds.contains_key(id)) {
            let mut worn = [[None; 4]; 2];
            for (models, look) in worn.iter_mut().zip([Some(&kind.adult), kind.baby.as_ref()]) {
                let Some(look) = look else { continue };
                for (to, slot) in models.iter_mut().zip(equipment::SLOTS) {
                    let Some(part) = layers.get(&look.layer.replace("#main", &format!("#{slot}"))) else { continue };
                    let id = model(mesh::bake(&Arc::new(part.clone())));
                    out.families.extend(Family::of_layer(look.layer).map(|f| (id, f)));
                    *to = Some(id);
                }
            }
            if worn.iter().flatten().any(Option::is_some) {
                out.armor.insert(identifier, worn);
            }
        }
        out.equipment = Equipment::load(root);
        out.equipment_textures = out.equipment.textures().filter_map(|path| Some((path.clone(), texture(&path)?))).collect();
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

    /// Whether `kind` draws a Java model.
    pub fn drawn(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind)
    }

    /// What armour `item` draws on a `kind` that draws a Java model, as one layer (a second texture
    /// is an overlay); posed by [`Library::pose`] on its own model. `dye` is the stack's.
    pub fn armor(&self, kind: &str, query: &dyn Fn(&str) -> Value, item: &str, dye: Option<[u8; 3]>) -> Option<Arc<[Layer]>> {
        let (_, own_baby) = self.kinds.get(kind)?;
        let baby = *own_baby && query("is_baby").truthy();
        let (slot, draws) = self.equipment.worn(item, baby, dye)?;
        let model = self.armor.get(kind)?[usize::from(baby)][slot]?;
        let mut textures = [NO_TEXTURE; 3];
        for (to, draw) in textures.iter_mut().zip(&draws) {
            *to = *self.equipment_textures.get(&draw.texture)?;
        }
        Some([Layer { textures, dye: draws.first()?.color, ..Layer::plain(model, NO_TEXTURE) }].into())
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
