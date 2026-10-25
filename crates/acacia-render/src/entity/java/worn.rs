//! The baked side of [`super::gear`]: its model layers and textures, and the layers a Bedrock
//! entity's gear adds.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use super::extras;
use super::gear::{self, Body, Gear};
use super::model::Part;
use super::{Family, mesh};
use crate::entity::bake::Mesh;
use crate::entity::molang::Value;
use crate::entity::{Layer, ModelId, TextureId};

#[derive(Default)]
pub struct Worn {
    models: HashMap<&'static str, ModelId>,
    textures: HashMap<String, TextureId>,
}

impl Worn {
    /// Bakes [`gear::LAYERS`] and [`extras::LAYERS`] (each with its family) and registers their
    /// textures.
    pub fn load(
        layers: &BTreeMap<String, Part>,
        model: &mut dyn FnMut(Mesh) -> ModelId,
        texture: &mut dyn FnMut(&str) -> Option<TextureId>,
        families: &mut HashMap<ModelId, Family>,
    ) -> Worn {
        let mut out = Worn::default();
        let all = gear::LAYERS.iter().map(|l| (*l, gear::family(l))).chain(extras::LAYERS.iter().map(|l| (*l, extras::family(l))));
        for (layer, family) in all {
            let Some(part) = layers.get(layer) else { continue };
            let id = model(mesh::bake(&Arc::new(part.clone())));
            families.extend(family.map(|f| (id, f)));
            out.models.insert(layer, id);
        }
        for path in gear::TEXTURES.iter().map(|name| gear::entity_texture(name)).chain(extras::textures()) {
            out.textures.extend(texture(&path).map(|t| (path, t)));
        }
        out
    }

    pub fn model(&self, layer: &str) -> Option<ModelId> {
        self.models.get(layer).copied()
    }

    pub fn texture(&self, path: &str) -> Option<TextureId> {
        self.textures.get(path).copied()
    }

    /// The layers `draws` make over a body drawn by `body_model`; `equipment` holds the equipment
    /// assets' textures by path.
    pub fn layers(&self, draws: Vec<gear::Draw>, body_model: ModelId, equipment: &HashMap<String, TextureId>) -> Vec<Layer> {
        let layer = |d: gear::Draw| {
            let model = d.layer.map_or(Some(body_model), |l| self.models.get(l).copied())?;
            let texture = *equipment.get(&d.texture).or_else(|| self.textures.get(&d.texture))?;
            Some(Layer { dye: d.color, blend: d.blend, fade: d.fade, ..Layer::plain(model, texture) })
        };
        draws.into_iter().filter_map(layer).collect()
    }
}

/// The gear a Bedrock entity's `query` and body slot `body` describe; `baby` as its model is.
pub fn bedrock_gear<'a>(query: &dyn Fn(&str) -> Value, baby: bool, body: Option<Body<'a>>) -> Gear<'a> {
    let flag = |name: &str| query(name).truthy();
    Gear {
        baby,
        invisible: flag("is_invisible"),
        saddled: flag("is_saddled"),
        body,
        collar: flag("is_tamed").then(|| query("color").num().max(0.0) as usize),
        markings: query("mark_variant").num().max(0.0) as usize,
        leash_holder: false,
    }
}
