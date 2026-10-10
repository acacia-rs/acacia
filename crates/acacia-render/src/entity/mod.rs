//! Entity models from the vanilla pack: geometries bake to rest-pose meshes, and each kind's
//! render controllers pick the meshes, textures and visible bones for an entity's state.
//! See README "Entities".

mod animation;
mod armor;
pub mod bake;
pub mod boat;
mod block_models;
mod controller;
pub mod geometry;
mod layer;
pub mod molang;
mod pose;
mod skin;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use acacia_molang::{Compiler, Symbol, Variable};
use glam::DVec3;

use crate::assets::image_file;
pub use bake::{Mesh, Vertex};
use controller::{Controller, Definition};
pub use layer::{Blend, Layer};
use molang::{Loading, Scope};
pub use molang::Value;
pub use pose::{BonePose, Pose};
pub use skin::{Skin, SkinSource};

pub type ModelId = u32;
pub type TextureId = u32;
pub const NO_TEXTURE: TextureId = u32::MAX;
/// A layer whose instance's skin brings the only mesh (items).
pub const NO_MODEL: ModelId = u32::MAX;

pub struct Model {
    pub mesh: Mesh,
}

const PLAYER_GEOMETRIES: [&str; 3] = ["geometry.humanoid.custom", "geometry.humanoid.customSlim", "geometry.humanoid"];
/// sRGB dye colours by the `color` data value, white first.
pub(crate) const DYES: [u32; 16] = [
    0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8, 0x3C44AA, 0x835432, 0x5E7C16,
    0xB02E26, 0x1D1D21,
];

fn dye(index: f32) -> [f32; 3] {
    let rgb = DYES[index.max(0.0) as usize % DYES.len()];
    [16, 8, 0].map(|shift| (((rgb >> shift) & 255) as f32 / 255.0).powf(2.2))
}

#[derive(Default)]
pub struct EntityModels {
    /// The directory loaded from; banner textures are read there as banners turn up.
    pub root: PathBuf,
    models: Vec<Model>,
    by_geometry: HashMap<String, ModelId>,
    textures: Vec<PathBuf>,
    /// Image path as definitions write it to its index in `textures`.
    texture_ids: HashMap<String, TextureId>,
    /// By kind identifier (`minecraft:cow`).
    kinds: HashMap<String, Definition>,
    controllers: HashMap<String, Controller>,
    animations: animation::Library,
    /// Owns the names the pack's compiled Molang shares.
    compiler: Compiler,
    /// [`pose::BUILT_IN`], by variable and the member's path in it.
    built_in: Vec<(Variable, Vec<Symbol>, f32)>,
    /// Wide arms, slim arms, 64×32 skin layout; drawn with Steve when the player has no skin.
    players: Option<[Arc<[Layer]>; 3]>,
    /// Worn armour by item identifier ([`armor`]).
    armor: HashMap<String, Arc<[Layer]>>,
}

/// One entity to draw this frame.
#[derive(Clone)]
pub struct EntityInstance {
    pub layers: Arc<[Layer]>,
    /// Replaces the layers' textures.
    pub skin: Option<Arc<Skin>>,
    /// Feet.
    pub position: DVec3,
    /// Of the body, in degrees as the server sends it.
    pub yaw: f32,
    pub scale: f32,
    /// From [`EntityModels::pose`]; the head's turn is part of it.
    pub pose: Pose,
    /// Model space to camera-relative world space, in place of position, yaw and scale: what the
    /// camera carries (the item in the player's hand). Lit at `position`.
    pub frame: Option<glam::Mat4>,
    /// Hurt or dying: drawn with the red overlay.
    pub hurt: bool,
    /// Enchanted: the glint shimmers over every layer.
    pub glint: Option<crate::glint::Glint>,
}

impl EntityModels {
    /// `root` holds the Bedrock pack's entity files: a resource pack or a look pack's files.
    pub fn load(root: &Path) -> EntityModels {
        // Packs newer than acacia-molang's query list still load: an unknown query reads 0.
        let mut compiler = Compiler::new();
        compiler.documented_queries_only = false;
        let loading = Loading::new(compiler);
        let mut out = EntityModels {
            root: root.to_owned(),
            kinds: controller::definitions(&loading, root),
            controllers: controller::controllers(&loading, root),
            animations: animation::load(&loading, root),
            ..Default::default()
        };
        out.compiler = loading.into_inner();
        let mut built_in = |&(name, value): &(&str, f32)| {
            let mut path = name.split('.');
            let variable = out.compiler.variable(path.next().unwrap_or_default());
            (variable, path.map(|member| out.compiler.symbol(member)).collect(), value)
        };
        out.built_in = pose::BUILT_IN.iter().map(&mut built_in).collect();
        let mut texture_sizes = HashMap::new();
        for d in out.kinds.values() {
            let texture = d.textures.get("default").or_else(|| d.textures.values().min());
            let Some((w, h)) = texture.and_then(|t| image::image_dimensions(image_file(root, t)?).ok()) else { continue };
            texture_sizes.extend(d.geometry.values().map(|g| (g.as_str(), [w as f32, h as f32])));
        }
        let armor = armor::pieces(root);
        for piece in &armor {
            if let Some((w, h)) = image_file(root, &piece.texture).and_then(|f| image::image_dimensions(f).ok()) {
                texture_sizes.insert(piece.geometry.as_str(), [w as f32, h as f32]);
            }
        }
        let geometries = geometry::load_all(root, &texture_sizes);
        let worn = armor.iter().map(|p| p.geometry.as_str());
        let masks = geometries.keys().map(String::as_str).filter(|id| id.ends_with(boat::MASK));
        let used = out.kinds.values().flat_map(|d| d.geometry.values()).map(String::as_str).chain(PLAYER_GEOMETRIES).chain(worn).chain(masks);
        for id in used {
            if let Some(geometry) = geometries.get(id).filter(|_| !out.by_geometry.contains_key(id)) {
                out.by_geometry.insert(id.to_owned(), out.models.len() as ModelId);
                out.models.push(Model { mesh: bake::bake(geometry) });
            }
        }
        for (id, mesh) in block_models::meshes(&geometries) {
            out.by_geometry.insert(id.to_owned(), out.models.len() as ModelId);
            out.models.push(Model { mesh });
        }
        let block_textures = block_models::textures(root);
        let armor_textures = armor.iter().map(|p| &p.texture);
        for path in out.kinds.values().flat_map(|d| d.textures.values()).chain(&block_textures).chain(armor_textures) {
            if let Some(file) = image_file(root, path).filter(|_| !out.texture_ids.contains_key(path)) {
                out.texture_ids.insert(path.clone(), out.textures.len() as TextureId);
                out.textures.push(file);
            }
        }
        let steve = out.kinds.get("minecraft:player").and_then(|d| out.texture_ids.get(d.textures.get("default")?)).copied();
        let player = |geometry: &str| -> Option<Arc<[Layer]>> {
            let model = *out.by_geometry.get(geometry)?;
            Some([Layer::plain(model, steve.unwrap_or(NO_TEXTURE))].into())
        };
        out.armor = armor
            .iter()
            .filter_map(|p| {
                let layer = Layer::plain(*out.by_geometry.get(&p.geometry)?, *out.texture_ids.get(&p.texture)?);
                Some((p.item.clone(), Arc::from([layer])))
            })
            .collect();
        let players = PLAYER_GEOMETRIES.map(player);
        out.players = players.iter().all(Option::is_some).then(|| players.map(Option::unwrap));
        let animations = out.animations.animations.len();
        tracing::info!(kinds = out.kinds.len(), models = out.models.len(), controllers = out.controllers.len(), animations, "entity models");
        out
    }

    pub fn models(&self) -> &[Model] {
        &self.models
    }

    /// What armour item `item` (`minecraft:diamond_helmet`) draws when worn, posed as its wearer.
    pub fn armor(&self, item: &str) -> Option<Arc<[Layer]>> {
        self.armor.get(item).cloned()
    }

    pub fn textures(&self) -> &[PathBuf] {
        &self.textures
    }

    /// A fresh scope for one entity, with the variables the game sets itself.
    fn scope<'a>(&'a self, query: &'a dyn Fn(&str) -> Value) -> Scope<'a> {
        let mut scope = Scope::new(&self.compiler, query);
        for (variable, path, value) in &self.built_in {
            scope.set(*variable, path, *value);
        }
        scope
    }

    /// What to draw for an entity of `kind` and the scale its definition asks for. `query`
    /// answers Molang queries about the entity by name (`is_baby`, `variant`); see
    /// [`molang::Scope::query`].
    pub fn appearance(&self, kind: &str, query: &dyn Fn(&str) -> Value) -> Option<(Arc<[Layer]>, f32)> {
        let definition = self.kinds.get(kind)?;
        let mut scope = self.scope(query);
        for script in &definition.scripts {
            scope.run(script);
        }
        let scale = definition.scale.as_ref().map_or(1.0, |s| scope.num(s));
        let mut layers = Vec::new();
        for (id, condition) in &definition.controllers {
            if condition.as_ref().is_some_and(|c| !scope.truthy(c)) {
                continue;
            }
            let Some(controller) = self.controllers.get(id) else { continue };
            layers.extend(self.layer(definition, controller, &mut scope));
        }
        if layers.is_empty() {
            layers.extend(self.plain(definition));
        }
        layers.extend(self.mask(definition).filter(|_| !layers.is_empty()));
        (!layers.is_empty()).then(|| (layers.into(), scale))
    }

    fn layer(&self, definition: &Definition, controller: &Controller, scope: &mut Scope) -> Option<Layer> {
        // `texture.default` to what the definition's `default` texture names.
        let named = |resource: Option<&str>, table: &HashMap<String, String>| table.get(resource?.split_once('.')?.1).cloned();
        let model = *self.by_geometry.get(&named(scope.resource(&controller.geometry), &definition.geometry)?)?;
        let material = controller.material.as_ref().and_then(|m| named(scope.resource(m), &definition.materials)).unwrap_or_default();
        let blend = Blend::of(&material)?;
        let mut textures = [NO_TEXTURE; 3];
        for (slot, texture) in textures.iter_mut().zip(&controller.textures) {
            let path = named(scope.resource(texture), &definition.textures);
            *slot = path.and_then(|p| self.texture_ids.get(&p).copied()).unwrap_or(NO_TEXTURE);
        }
        let rules: Vec<(&str, bool)> = controller.part_visibility.iter().map(|(bone, v)| (bone.as_str(), scope.truthy(v))).collect();
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
        (textures[0] != NO_TEXTURE && shown > 0).then_some(Layer { model, textures, tint, dye: None, hidden, blend })
    }

    /// The default geometry in the default texture, for kinds whose controllers yield nothing.
    fn plain(&self, definition: &Definition) -> Option<Layer> {
        let pick = |table: &HashMap<String, String>| table.get("default").or_else(|| table.values().min()).cloned();
        let model = *self.by_geometry.get(&pick(&definition.geometry)?)?;
        let texture = *self.texture_ids.get(&pick(&definition.textures)?)?;
        Some(Layer::plain(model, texture))
    }

    /// The one layer of a block model ([`crate::blocks::model`]), if its geometry and texture loaded.
    pub fn block_layers(&self, geometry: &str, texture: &str) -> Option<Arc<[Layer]>> {
        let (model, texture) = (*self.by_geometry.get(geometry)?, *self.texture_ids.get(texture)?);
        Some([Layer::plain(model, texture)].into())
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
