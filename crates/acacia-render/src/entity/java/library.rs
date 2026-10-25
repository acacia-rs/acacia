//! The Java models a look carries (`java/entity_models.json`, written by `lookbake java`), baked
//! for the kinds in [`super::kinds`]; [`crate::entity::EntityModels`] asks it first.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::equipment::{self, Equipment};
use super::kinds::{self, Look, When};
use super::trims::{Trim, Trims};
use super::state::Field;
use super::variants;
use super::extras;
use super::villager::Professions;
use super::worn::{self, Worn};
use super::gear::{self, Body};
use super::{Family, Poses, anim, bedrock, mesh};
use crate::entity::bake::Mesh;
use crate::entity::molang::Value;
use crate::entity::{Blend, DYES, Layer, ModelId, NO_TEXTURE, Pose, TextureId};

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
    trims: Trims,
    /// Equipment and trim textures by path.
    equipment_textures: HashMap<String, TextureId>,
    /// By Bedrock identifier, the second layers' adult and own-baby (model, texture).
    overlays: HashMap<&'static str, Vec<(When, [Option<(ModelId, TextureId)>; 2])>>,
    /// [`variants::VARIANTS`]' adult and baby textures.
    variants: HashMap<&'static str, Vec<(&'static str, i32, [TextureId; 2])>>,
    professions: HashMap<&'static str, Professions>,
    worn: Worn,
}

/// The kinds `VillagerProfessionLayer` dresses: their texture directory and `setupAnim`.
const PROFESSION_KINDS: [(&str, &str, Family); 4] = [
    ("minecraft:villager_v2", "villager", Family::Villager),
    ("minecraft:villager", "villager", Family::Villager),
    ("minecraft:zombie_villager_v2", "zombie_villager", Family::Zombie),
    ("minecraft:zombie_villager", "zombie_villager", Family::Zombie),
];

/// `ColorLerper.Type.SHEEP`'s colour of the wool dyed `color` (white first): white is 0xE6E6E6,
/// any other dye's diffuse colour at 3/4.
fn wool(color: usize) -> [u8; 3] {
    if color == 0 {
        return [0xE6; 3];
    }
    [16, 8, 0].map(|shift| ((((DYES[color] >> shift) & 255) as f32) * 0.75).floor() as u8)
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
        super::keyframes::install(root);
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
        for (identifier, overlay) in &kinds::OVERLAYS {
            let Some(kind) = kinds::of(identifier).filter(|_| out.kinds.contains_key(identifier)) else { continue };
            let mut pair = [None; 2];
            for (to, (look, path)) in pair.iter_mut().zip([(Some(&kind.adult), Some(overlay.texture)), (kind.baby.as_ref(), overlay.baby_texture)]) {
                let (Some(look), Some(path)) = (look, path) else { continue };
                let Some(part) = layers.get(&look.layer.replace("#main", &format!("#{}", overlay.suffix))) else { continue };
                let Some(texture) = texture(&format!("java/textures/entity/{path}")) else { continue };
                let id = model(mesh::bake(&Arc::new(part.clone())));
                out.families.extend(Family::of_layer(look.layer).map(|f| (id, f)));
                *to = Some((id, texture));
            }
            out.overlays.entry(identifier).or_default().push((overlay.when, pair));
        }
        for (identifier, query, value, adult, baby) in variants::VARIANTS {
            let mut path = |p: &str| texture(&format!("java/textures/entity/{p}"));
            let Some(adult) = path(adult) else { continue };
            out.variants.entry(identifier).or_default().push((query, value, [adult, path(baby).unwrap_or(adult)]));
        }
        for (identifier, dir, family) in PROFESSION_KINDS {
            let mut no_hat = [None; 2];
            for (to, layer) in no_hat.iter_mut().zip([format!("minecraft:{dir}_no_hat#main"), format!("minecraft:{dir}_baby_no_hat#main")]) {
                let Some(part) = layers.get(&layer) else { continue };
                let id = model(mesh::bake(&Arc::new(part.clone())));
                out.families.insert(id, family);
                *to = Some(id);
            }
            out.professions.insert(identifier, Professions::load(root, dir, no_hat, &mut *texture));
        }
        out.equipment = Equipment::load(root);
        out.trims = Trims::load(root);
        let paths = out.equipment.textures().chain(Trims::images(root));
        out.equipment_textures = paths.filter_map(|path| Some((path.clone(), texture(&path)?))).collect();
        out.worn = Worn::load(&layers, model, texture, &mut out.families);
        tracing::info!(kinds = out.kinds.len(), layers = layers.len(), "java entity models");
        out
    }

    /// The layer and scale for `kind`, when it draws a Java model. A baby with its own model is
    /// drawn at full size, undoing the 0.5 the server sends (README "Java entity models").
    pub fn appearance(&self, kind: &str, query: &dyn Fn(&str) -> Value) -> Option<(Arc<[Layer]>, f32)> {
        let (variants, own_baby) = self.kinds.get(kind)?;
        let baby = query("is_baby").truthy();
        let (model, texture) = variants[usize::from(baby)];
        let variant = self.variants.get(kind).into_iter().flatten().find(|(q, value, _)| variants::matches(q, *value, query));
        let texture = variant.map_or(texture, |(_, _, t)| t[usize::from(baby && *own_baby)]);
        let scale = if baby && *own_baby { 2.0 } else { 1.0 };
        let java = kinds::java_id(kind);
        let mut body = Layer::plain(model, texture);
        if let Some(skin) = extras::skin(java, query) {
            body.model = skin.layer.and_then(|l| self.worn.model(l)).unwrap_or(model);
            body.textures[0] = skin.texture.and_then(|p| self.worn.texture(&p)).unwrap_or(texture);
            body.dye = skin.color;
            body.blend = skin.blend;
        }
        let model = body.model;
        let mut layers = vec![body];
        if let Some(professions) = self.professions.get(kind) {
            layers.extend(professions.layers(model, baby && *own_baby, query));
        }
        let color = query("color").num().max(0.0) as usize % DYES.len();
        for (when, pair) in self.overlays.get(kind).into_iter().flatten() {
            let Some((model, texture)) = pair[usize::from(baby && *own_baby)] else { continue };
            let plain = Layer::plain(model, texture);
            layers.push(match when {
                When::Always => plain,
                When::Wool if !query("is_sheared").truthy() => Layer { dye: Some(wool(color)), ..plain },
                When::Undercoat if !baby && color != 0 => Layer { dye: Some(wool(color)), ..plain },
                When::Powered if query("is_powered").truthy() => Layer { blend: Blend::Swirl, ..plain },
                When::Eyes => Layer { blend: Blend::Eyes, ..plain },
                When::Translucent => Layer { blend: Blend::Alpha, ..plain },
                _ => continue,
            });
        }
        layers.extend(self.worn.layers(extras::draws(java, query), model, &self.equipment_textures));
        Some((layers.into(), scale))
    }

    /// Whether `id` is a Java second layer or armour piece, posed on its own ([`Library::pose`]).
    pub fn posed_apart(&self, body: ModelId, id: ModelId) -> bool {
        id != body && self.families.contains_key(&id)
    }

    /// Whether `kind` draws a Java model.
    pub fn drawn(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind)
    }

    /// What armour `item` draws on a `kind` that draws a Java model, as one layer (a second texture
    /// is an overlay); posed by [`Library::pose`] on its own model. `dye` and `trim` are the stack's.
    pub fn armor(&self, kind: &str, query: &dyn Fn(&str) -> Value, item: &str, dye: Option<[u8; 3]>, trim: Option<&Trim>) -> Option<Arc<[Layer]>> {
        let (_, own_baby) = self.kinds.get(kind)?;
        let baby = *own_baby && query("is_baby").truthy();
        let (slot, draws) = self.equipment.worn(item, baby, dye, trim.map(|t| (&self.trims, t)))?;
        let model = self.armor.get(kind)?[usize::from(baby)][slot]?;
        let mut textures = [NO_TEXTURE; 3];
        for (to, draw) in textures.iter_mut().zip(&draws) {
            *to = *self.equipment_textures.get(&draw.texture)?;
        }
        Some([Layer { textures, dye: draws.first()?.color, ..Layer::plain(model, NO_TEXTURE) }].into())
    }

    /// The layers a `kind`'s saddle, collar, markings and body-slot stack `body` add ([`gear`]).
    pub fn gear(&self, kind: &str, query: &dyn Fn(&str) -> Value, body: Option<Body>) -> Vec<Layer> {
        let Some(([adult, baby_model], own_baby)) = self.kinds.get(kind) else { return Vec::new() };
        let baby = query("is_baby").truthy();
        let body_model = if baby && *own_baby { baby_model.0 } else { adult.0 };
        let draws = gear::draws(kinds::java_id(kind), &worn::bedrock_gear(query, baby, body), &self.equipment);
        self.worn.layers(draws, body_model, &self.equipment_textures)
    }

    /// The whole-body transform of a `kind` drawn by a Java model ([`super::body`]), in the entity
    /// pass's model space: turned by `yaw`, scaled by `scale`, toppled `death_ticks` into dying.
    pub fn body(&self, kind: &str, query: &dyn Fn(&str) -> Value, yaw: f32, scale: f32, death_ticks: f32) -> Option<glam::Mat4> {
        let family = Family::of_layer(kinds::of(kind)?.adult.layer)?;
        let java = kinds::java_id(kind);
        let mut state = bedrock::render_state(family, query);
        state.set("bodyRot", Field::Num(yaw)).set("scale", Field::Num(scale)).set("deathTime", Field::Num(death_ticks));
        // Bedrock's scale already carries a cube's size.
        state.set("size", Field::Num(if matches!(java, "slime" | "magma_cube") { 1.0 } else { 0.0 }));
        let java_frame = super::body::transform(java, &state);
        // The mesh's own root (mesh.rs) is the flip and the drop; take them back off.
        Some(java_frame * glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.501, 0.0)) * glam::Mat4::from_scale(glam::Vec3::new(1.0, -1.0, 1.0)))
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

#[cfg(test)]
mod tests {
    #[test]
    fn wool_is_the_dye_at_three_quarters_and_white_is_grey() {
        assert_eq!(super::wool(0), [0xE6; 3]);
        // Orange, 0xF9801D.
        assert_eq!(super::wool(1), [186, 96, 21]);
    }
}
