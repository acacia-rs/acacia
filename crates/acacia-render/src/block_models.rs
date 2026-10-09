//! The loaded sections' block models ([`crate::blocks::model`]) as entity instances.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{DVec3, IVec3};
use rustc_hash::FxHashMap;

use crate::blocks::model::{BlockData, BlockModel};
use crate::entity::{EntityInstance, EntityModels, Layer, Pose};
use crate::sign_text::{self, SignTextMap};
use crate::workers::SectionKey;

/// Block entity data by block position.
pub type BlockDataMap = HashMap<[i32; 3], BlockData>;

/// A section's block models by position within the section.
pub type SectionModels = Vec<([u8; 3], Arc<BlockModel>)>;

#[derive(Default)]
pub struct BlockModels {
    sections: FxHashMap<SectionKey, SectionModels>,
    data: Arc<BlockDataMap>,
    models: Arc<EntityModels>,
    instances: Vec<EntityInstance>,
    text: Arc<SignTextMap>,
    signs: Vec<sign_text::Placed>,
    stale: bool,
}

impl BlockModels {
    /// Replaces a section's models; an empty list forgets the section.
    pub fn set_section(&mut self, key: SectionKey, models: SectionModels) {
        self.stale |= match models.is_empty() {
            true => self.sections.remove(&key).is_some(),
            false => {
                self.sections.insert(key, models);
                true
            }
        };
    }

    pub fn clear(&mut self) {
        self.sections.clear();
        self.stale = true;
    }

    pub fn set_data(&mut self, data: Arc<BlockDataMap>) {
        self.data = data;
        self.stale = true;
    }

    pub fn set_models(&mut self, models: Arc<EntityModels>) {
        self.models = models;
        self.stale = true;
    }

    pub fn set_sign_text(&mut self, text: Arc<SignTextMap>) {
        self.text = text;
        self.stale = true;
    }

    /// The loaded signs' text, as of the last [`Self::near`].
    pub fn sign_text(&self) -> &[sign_text::Placed] {
        &self.signs
    }

    /// Instances within `reach` blocks of `camera`.
    pub fn near(&mut self, camera: DVec3, reach: f64) -> impl Iterator<Item = &EntityInstance> {
        if std::mem::take(&mut self.stale) {
            self.rebuild();
        }
        self.instances.iter().filter(move |e| e.position.distance_squared(camera) < reach * reach)
    }

    fn rebuild(&mut self) {
        let mut layers: HashMap<(&str, String), Option<Arc<[Layer]>>> = HashMap::new();
        self.instances.clear();
        self.signs.clear();
        for (&(cx, sy, cz), models) in &self.sections {
            for (local, model) in models {
                let pos = IVec3::new(cx, sy, cz) * 16 + IVec3::from(local.map(i32::from));
                if let Some((text, faces)) = self.text.get(&pos.to_array()).zip(sign_text::transforms(model)) {
                    self.signs.push(sign_text::Placed { block: pos, faces, text: text.clone() });
                }
                let Some(placed) = model.place(pos, self.data.get(&pos.to_array())) else { continue };
                let look = layers.entry((placed.geometry, placed.texture)).or_insert_with_key(|(geometry, texture)| self.models.block_layers(geometry, texture));
                let Some(layers) = look.clone() else { continue };
                self.instances.push(EntityInstance { layers, skin: None, position: placed.position, yaw: placed.yaw, scale: 1.0, pose: Pose::default(), frame: None, hurt: false });
            }
        }
    }
}
