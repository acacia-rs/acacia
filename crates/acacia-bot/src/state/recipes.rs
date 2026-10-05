//! Recipes from the server's `CraftingData`. The packet is large, so the tracker keeps it raw and
//! decodes it the first time a recipe is looked up: bots that never craft never pay for it.

use std::collections::HashMap;
use std::sync::OnceLock;

use acacia_client::proto::packets::CraftingData;
use acacia_client::proto::{Packet, RawPacket};

use super::ItemStack;

mod decode;
mod molang;
mod tags;

/// What a recipe is crafted at and how its inputs are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeKind {
    /// `inputs` holds `width * height` cells row by row; blank cells are [`Ingredient::Empty`].
    Shaped { width: u8, height: u8 },
    Shapeless,
    /// `inputs` is `[template, base, addition]`, `outputs` the upgraded item.
    SmithingTransform,
    /// `inputs` is `[template, base, addition]`; the result is the base with a trim, so no outputs.
    SmithingTrim,
}

/// One recipe; `network_id` is what `CraftRecipe` actions refer to.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub network_id: u32,
    pub id: String,
    pub kind: RecipeKind,
    /// The station tag: `crafting_table`, `stonecutter`, `smithing_table`, `cartography_table`,
    /// `furnace`, ... (a 2x2 recipe also says `crafting_table`).
    pub block: String,
    pub priority: i32,
    pub inputs: Vec<Ingredient>,
    pub outputs: Vec<ItemStack>,
}

impl Recipe {
    /// Fits the player's own 2x2 grid.
    pub fn fits_2x2(&self) -> bool {
        match self.kind {
            RecipeKind::Shaped { width, height } => width <= 2 && height <= 2,
            RecipeKind::Shapeless => self.inputs.iter().filter(|i| !i.is_empty()).count() <= 4,
            _ => false,
        }
    }
}

/// A recipe input slot.
#[derive(Debug, Clone, PartialEq)]
pub enum Ingredient {
    Empty,
    /// `metadata` `None` takes any aux value.
    Item { name: String, metadata: Option<i32>, count: u16 },
    Tag { tag: String, count: u16 },
    Molang { expression: String, version: i16, count: u16 },
}

impl Ingredient {
    pub fn is_empty(&self) -> bool {
        matches!(self, Ingredient::Empty) || self.count() == 0
    }

    pub fn count(&self) -> u16 {
        match self {
            Ingredient::Empty => 0,
            Ingredient::Item { count, .. } | Ingredient::Tag { count, .. } | Ingredient::Molang { count, .. } => *count,
        }
    }

    /// Whether an item with identifier `name` and aux `metadata` fills this ingredient. Tags, also
    /// those a MoLang expression asks about, are resolved from a built-in table of vanilla tags
    /// (`tags.rs`).
    pub fn accepts(&self, name: &str, metadata: u32) -> bool {
        match self {
            Ingredient::Empty => false,
            Ingredient::Item { name: want, metadata: meta, .. } => want == name && meta.is_none_or(|m| m as u32 == metadata),
            Ingredient::Tag { tag, .. } => tags::has_tag(name, tag),
            Ingredient::Molang { expression, .. } => molang::accepts(expression, name),
        }
    }
}

/// Every decoded recipe, indexed by network id.
#[derive(Debug, Default)]
pub struct RecipeBook {
    recipes: Vec<Recipe>,
    by_network_id: HashMap<u32, usize>,
    /// Network ids of the built-in multi recipes (map cloning, banner duplication, ...) by UUID.
    multi: HashMap<String, u32>,
}

impl RecipeBook {
    pub fn get(&self, network_id: u32) -> Option<&Recipe> {
        self.by_network_id.get(&network_id).map(|&i| &self.recipes[i])
    }

    /// Network id of the multi recipe with `uuid` (lowercase, dashed).
    pub fn multi_recipe(&self, uuid: &str) -> Option<u32> {
        self.multi.get(uuid).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.iter()
    }

    /// Recipes whose first output is item `network_id`, in the server's order.
    pub fn producing(&self, network_id: i32) -> impl Iterator<Item = &Recipe> {
        self.recipes.iter().filter(move |r| r.outputs.first().is_some_and(|o| o.network_id == network_id))
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    fn clear(&mut self) {
        self.recipes.clear();
        self.by_network_id.clear();
        self.multi.clear();
    }

    fn push(&mut self, recipe: Recipe) {
        self.by_network_id.insert(recipe.network_id, self.recipes.len());
        self.recipes.push(recipe);
    }
}

/// The server's recipes. `CraftingData` packets are kept undecoded until [`Recipes::book`].
#[derive(Debug, Default)]
pub struct Recipes {
    packets: Vec<RawPacket>,
    book: OnceLock<RecipeBook>,
}

impl Recipes {
    pub const PACKETS: &'static [u32] = &[CraftingData::ID];

    pub fn apply(&mut self, packet: &RawPacket) {
        self.packets.push(packet.clone());
        self.book = OnceLock::new();
    }

    /// The recipes, decoding the kept packets on first use (a packet that fails to decode is
    /// skipped and logged).
    pub fn book(&self) -> &RecipeBook {
        self.book.get_or_init(|| {
            let mut book = RecipeBook::default();
            for packet in &self.packets {
                match packet.decode::<CraftingData>() {
                    Ok(data) => decode::add(&mut book, data),
                    Err(e) => tracing::debug!(error = %e, "cannot decode CraftingData"),
                }
            }
            book
        })
    }

    /// Whether the server sent any `CraftingData` yet (without decoding it).
    pub fn received(&self) -> bool {
        !self.packets.is_empty()
    }
}

#[cfg(test)]
mod tests;
