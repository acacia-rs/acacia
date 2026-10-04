//! Entity models from the vanilla pack: geometries bake to rest-pose meshes, and each kind's
//! render controllers pick the meshes, textures and visible bones for an entity's state.
//! See README "Entities".

pub mod bake;
mod controller;
pub mod geometry;
pub mod molang;
mod molang_parse;
mod skin;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec3;

use crate::assets::Pack;
pub use bake::{Mesh, Vertex};
use controller::{Controller, Definition};
use molang::Scope;
pub use molang::Value;
pub use skin::{Skin, SkinSource};

pub type ModelId = u32;
pub type TextureId = u32;
pub const NO_TEXTURE: TextureId = u32::MAX;

pub struct Model {
    pub mesh: Mesh,
}

/// One draw of an entity: a render controller's choice of mesh and textures.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub model: ModelId,
    /// Laid over each other, bottom first; unused slots hold [`NO_TEXTURE`].
    pub textures: [TextureId; 3],
    /// Linear colour multiplied in where the texture's alpha is 0 (sheep wool).
    pub tint: Option<[f32; 3]>,
    /// Bit per bone of [`Mesh::bones`] that is not drawn.
    pub hidden: [u32; 4],
}

/// Materials drawn blended over the body (slime shell, charged creeper aura, enchantment glint);
/// the entity pass is opaque, so their layers are left out.
const OVERLAY_MATERIALS: [&str; 8] = ["outer", "charged", "ghost", "wind", "bioluminescent", "dissolve", "spectator", "enchanted"];
const PLAYER_GEOMETRIES: [&str; 3] = ["geometry.humanoid.custom", "geometry.humanoid.customSlim", "geometry.humanoid"];
/// sRGB dye colours by the `color` data value, white first.
const DYES: [u32; 16] = [
    0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8, 0x3C44AA, 0x835432, 0x5E7C16,
    0xB02E26, 0x1D1D21,
];

fn dye(index: f32) -> [f32; 3] {
    let rgb = DYES[index.max(0.0) as usize % DYES.len()];
    [16, 8, 0].map(|shift| (((rgb >> shift) & 255) as f32 / 255.0).powf(2.2))
}

#[derive(Default)]
pub struct EntityModels {
    models: Vec<Model>,
    by_geometry: HashMap<String, ModelId>,
    textures: Vec<PathBuf>,
    /// Image path as definitions write it to its index in `textures`.
    texture_ids: HashMap<String, TextureId>,
    /// By kind identifier (`minecraft:cow`).
    kinds: HashMap<String, Definition>,
    controllers: HashMap<String, Controller>,
    /// Wide arms, slim arms, 64×32 skin layout; drawn with Steve when the player has no skin.
    players: Option<[Arc<[Layer]>; 3]>,
}

/// One entity to draw this frame. Angles in degrees, as the server sends them.
#[derive(Clone)]
pub struct EntityInstance {
    pub layers: Arc<[Layer]>,
    /// Replaces the layers' textures.
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
        let mut out = EntityModels { kinds: controller::definitions(pack.root()), controllers: controller::controllers(pack.root()), ..Default::default() };
        let mut texture_sizes = HashMap::new();
        for d in out.kinds.values() {
            let texture = d.textures.get("default").or_else(|| d.textures.values().min());
            let Some((w, h)) = texture.and_then(|t| image::image_dimensions(pack.image_file(t)?).ok()) else { continue };
            texture_sizes.extend(d.geometry.values().map(|g| (g.as_str(), [w as f32, h as f32])));
        }
        let geometries = geometry::load_all(pack.root(), &texture_sizes);
        let used = out.kinds.values().flat_map(|d| d.geometry.values()).map(String::as_str).chain(PLAYER_GEOMETRIES);
        for id in used {
            if let Some(geometry) = geometries.get(id).filter(|_| !out.by_geometry.contains_key(id)) {
                out.by_geometry.insert(id.to_owned(), out.models.len() as ModelId);
                out.models.push(Model { mesh: bake::bake(geometry) });
            }
        }
        for path in out.kinds.values().flat_map(|d| d.textures.values()) {
            if let Some(file) = pack.image_file(path).filter(|_| !out.texture_ids.contains_key(path)) {
                out.texture_ids.insert(path.clone(), out.textures.len() as TextureId);
                out.textures.push(file);
            }
        }
        let steve = out.kinds.get("minecraft:player").and_then(|d| out.texture_ids.get(d.textures.get("default")?)).copied();
        let player = |geometry: &str| -> Option<Arc<[Layer]>> {
            let model = *out.by_geometry.get(geometry)?;
            Some([Layer { model, textures: [steve.unwrap_or(NO_TEXTURE), NO_TEXTURE, NO_TEXTURE], tint: None, hidden: [0; 4] }].into())
        };
        let players = PLAYER_GEOMETRIES.map(player);
        out.players = players.iter().all(Option::is_some).then(|| players.map(Option::unwrap));
        tracing::info!(kinds = out.kinds.len(), models = out.models.len(), controllers = out.controllers.len(), "entity models");
        out
    }

    pub fn models(&self) -> &[Model] {
        &self.models
    }

    pub fn textures(&self) -> &[PathBuf] {
        &self.textures
    }

    /// What to draw for an entity of `kind` and the scale its definition asks for. `query`
    /// answers Molang queries about the entity by name (`is_baby`, `variant`); see
    /// [`molang::Scope::query`].
    pub fn appearance(&self, kind: &str, query: &dyn Fn(&str) -> Value) -> Option<(Arc<[Layer]>, f32)> {
        let definition = self.kinds.get(kind)?;
        let mut scope = Scope { query, variables: HashMap::new(), arrays: None };
        for script in &definition.scripts {
            script.run(&mut scope);
        }
        let scale = definition.scale.as_ref().map_or(1.0, |s| s.run(&mut scope).num());
        let mut layers = Vec::new();
        for (id, condition) in &definition.controllers {
            scope.arrays = None;
            if condition.as_ref().is_some_and(|c| !c.run(&mut scope).truthy()) {
                continue;
            }
            let Some(controller) = self.controllers.get(id) else { continue };
            scope.arrays = Some(&controller.arrays);
            layers.extend(self.layer(definition, controller, &mut scope));
        }
        if layers.is_empty() {
            layers.extend(self.plain(definition));
        }
        (!layers.is_empty()).then(|| (layers.into(), scale))
    }

    fn layer(&self, definition: &Definition, controller: &Controller, scope: &mut Scope) -> Option<Layer> {
        // `texture.default` to what the definition's `default` texture names.
        let named = |value: Value, table: &HashMap<String, String>| match value {
            Value::Text(name) => table.get(name.split_once('.')?.1).cloned(),
            Value::Num(_) => None,
        };
        let model = *self.by_geometry.get(&named(controller.geometry.run(scope), &definition.geometry)?)?;
        let material = controller.material.as_ref().and_then(|m| named(m.run(scope), &definition.materials)).unwrap_or_default();
        if OVERLAY_MATERIALS.iter().any(|m| material.contains(m)) {
            return None;
        }
        let mut textures = [NO_TEXTURE; 3];
        for (slot, texture) in textures.iter_mut().zip(&controller.textures) {
            let path = named(texture.run(scope), &definition.textures);
            *slot = path.and_then(|p| self.texture_ids.get(&p).copied()).unwrap_or(NO_TEXTURE);
        }
        let rules: Vec<(&str, bool)> = controller.part_visibility.iter().map(|(bone, v)| (bone.as_str(), v.run(scope).truthy())).collect();
        let bones = &self.models[model as usize].mesh.bones;
        let mut hidden = [0u32; 4];
        let mut shown = bones.len();
        for (index, bone) in bones.iter().enumerate() {
            let visible = rules.iter().rev().find(|(pattern, _)| controller::matches(pattern, bone)).is_none_or(|&(_, v)| v);
            if !visible {
                shown -= 1;
                // The last index is shared by every bone past it: see `bake::MAX_BONES`.
                if index < bake::MAX_BONES - 1 {
                    hidden[index / 32] |= 1 << (index % 32);
                }
            }
        }
        let tint = (material == "sheep").then(|| dye((scope.query)("color").num()));
        (textures[0] != NO_TEXTURE && shown > 0).then_some(Layer { model, textures, tint, hidden })
    }

    /// The default geometry in the default texture, for kinds whose controllers yield nothing.
    fn plain(&self, definition: &Definition) -> Option<Layer> {
        let pick = |table: &HashMap<String, String>| table.get("default").or_else(|| table.values().min()).cloned();
        let model = *self.by_geometry.get(&pick(&definition.geometry)?)?;
        let texture = *self.texture_ids.get(&pick(&definition.textures)?)?;
        Some(Layer { model, textures: [texture, NO_TEXTURE, NO_TEXTURE], tint: None, hidden: [0; 4] })
    }

    /// The humanoid for a skin: slim or wide arms, or the old layout when the skin is half height.
    pub fn player(&self, skin: Option<(&Skin, bool)>) -> Option<Arc<[Layer]>> {
        let [wide, slim, legacy] = self.players.as_ref()?;
        Some(match skin {
            Some((s, _)) if s.height * 2 == s.width => legacy.clone(),
            Some((_, true)) => slim.clone(),
            _ => wide.clone(),
        })
    }
}
