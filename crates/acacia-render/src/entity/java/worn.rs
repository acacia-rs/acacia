//! The baked side of [`super::gear`]: its model layers and textures, and the layers a Bedrock
//! entity's gear adds.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

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
    /// Bakes [`gear::LAYERS`] (each with its [`gear::family`]) and registers [`gear::TEXTURES`].
    pub fn load(
        layers: &BTreeMap<String, Part>,
        model: &mut dyn FnMut(Mesh) -> ModelId,
        texture: &mut dyn FnMut(&str) -> Option<TextureId>,
        families: &mut HashMap<ModelId, Family>,
    ) -> Worn {
        let mut out = Worn::default();
        for layer in gear::LAYERS {
            let Some(part) = layers.get(layer) else { continue };
            let id = model(mesh::bake(&Arc::new(part.clone())));
            families.extend(gear::family(layer).map(|f| (id, f)));
            out.models.insert(layer, id);
        }
        for name in gear::TEXTURES {
            let path = gear::entity_texture(name);
            out.textures.extend(texture(&path).map(|t| (path, t)));
        }
        out
    }

    /// The layers `draws` make over a body drawn by `body_model`; `equipment` holds the equipment
    /// assets' textures by path.
    pub fn layers(&self, draws: Vec<gear::Draw>, body_model: ModelId, equipment: &HashMap<String, TextureId>) -> Vec<Layer> {
        let layer = |d: gear::Draw| {
            let model = d.layer.map_or(Some(body_model), |l| self.models.get(l).copied())?;
            let texture = *equipment.get(&d.texture).or_else(|| self.textures.get(&d.texture))?;
            Some(Layer { dye: d.color, blend: d.blend, ..Layer::plain(model, texture) })
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
