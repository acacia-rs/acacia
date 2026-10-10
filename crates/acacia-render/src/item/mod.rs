//! Items as the world shows them: a flat item is its icon extruded, a block item is its block.
//! See README "Items".

mod block;
pub mod drop;
mod extrude;
pub mod hand;
mod icon;
mod icons;
pub mod leather;
mod shield;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::blocks::{BlockTable, Shape};
use crate::entity::{Layer, NO_MODEL, NO_TEXTURE, Skin};
use crate::LookPack;
use crate::look::Dropped;
pub use icon::{block_icon, model_icon, pattern_icon};
pub use icons::ItemIcons;

/// An item stack as the server names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemKey {
    /// `minecraft:apple`.
    pub name: String,
    /// Damage or variant.
    pub aux: u32,
    /// Block runtime id in the world's registry; 0 for items that are not blocks.
    pub block: u32,
    /// Dyed leather's colour ([`leather`]).
    pub dye: Option<[u8; 3]>,
}

/// What a model is, for how it is held, dropped and stacked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// An extruded sprite.
    Flat,
    /// A unit cube's worth.
    Block,
    /// The shield's own model ([`shield`]).
    Shield,
}

/// An item's mesh and texture, drawn through the entity pass like a persona skin.
#[derive(Clone)]
pub struct ItemModel {
    pub skin: Arc<Skin>,
    /// One layer that draws nothing of its own, so the skin's mesh is all there is.
    pub layers: Arc<[Layer]>,
    pub form: Form,
    /// Shimmers: the stack is enchanted, or the look's game always draws the item so.
    pub glint: bool,
}

pub struct ItemModels {
    icons: ItemIcons,
    pack: Arc<LookPack>,
    table: Arc<BlockTable>,
    layers: Arc<[Layer]>,
    cache: HashMap<ItemKey, Option<ItemModel>>,
}

impl ItemModels {
    /// `table` is the look pack's for the world's registry ([`LookPack::block_table`]).
    pub fn new(pack: Arc<LookPack>, table: Arc<BlockTable>) -> ItemModels {
        let layers = [Layer::plain(NO_MODEL, NO_TEXTURE)].into();
        ItemModels { icons: ItemIcons::load(pack.files()), pack, table, layers, cache: HashMap::new() }
    }

    /// How the look's dropped items move.
    pub fn dropped(&self) -> &Dropped {
        &self.pack.look.dropped
    }

    /// Built on first use and kept; `None` for items with neither an icon nor a drawable block.
    /// `enchanted` is the stack's own ([`ItemModel::glint`]).
    pub fn get(&mut self, key: &ItemKey, enchanted: bool) -> Option<ItemModel> {
        if !self.cache.contains_key(key) {
            let model = self.build(key);
            if model.is_none() {
                tracing::debug!(item = key.name, aux = key.aux, "item without a model");
            }
            self.cache.insert(key.clone(), model);
        }
        let model = self.cache.get(key)?.clone()?;
        Some(ItemModel { glint: model.glint || enchanted, ..model })
    }

    fn build(&self, key: &ItemKey) -> Option<ItemModel> {
        let glint = self.pack.look.foil.always(&key.name);
        let model = |skin: Skin, form| Some(ItemModel { skin: Arc::new(skin), layers: self.layers.clone(), form, glint });
        if let Some(skin) = Some(self.pack.files()).filter(|_| key.name == shield::ITEM).and_then(shield::skin) {
            return model(skin, Form::Shield);
        }
        // An item with an icon of its own shows it, even when it places a block (doors, beds).
        let dye = leather::dyeable(&key.name).then_some(key.dye);
        if let Some(skin) = self.icons.path(&key.name, key.aux).and_then(|path| icon(self.pack.files(), path, dye)) {
            return model(skin, Form::Flat);
        }
        if key.block == 0 {
            return None;
        }
        let block = self.table.get(key.block);
        if block.shape == Shape::Cross {
            let rgba = block::texels(self.pack.atlas.layers.get(block.textures[0] as usize), block::tile_of(block, 0));
            return model(flat(16, 16, rgba), Form::Flat);
        }
        model(block::skin(block, &self.pack.atlas)?, Form::Block)
    }
}

/// The icon's first frame (strips stack frames downwards) as an extruded sprite; a leather
/// piece's with its `dye` (`Some(None)` undyed).
fn icon(root: &Path, path: &str, dye: Option<Option<[u8; 3]>>) -> Option<Skin> {
    let image = leather::icon(root, path, dye)?;
    let (width, height) = (image.width(), image.height().min(image.width()));
    let rgba = image.into_raw()[..(width * height * 4) as usize].to_vec();
    Some(flat(width, height, rgba))
}

fn flat(width: u32, height: u32, rgba: Vec<u8>) -> Skin {
    let mesh = extrude::extrude(width, height, &rgba);
    Skin { width, height, rgba, mesh: Some(mesh) }
}
