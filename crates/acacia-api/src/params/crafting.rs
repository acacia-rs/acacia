//! Params and results of crafting and smelting.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BlockPos, BotId};

/// The item to craft, how many and where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CraftParams {
    pub bot: BotId,
    /// The item to make (`minecraft:stick`, or just `stick`).
    pub item: String,
    /// How many items at least; 1 when absent. A craft makes a recipe's whole output (4 sticks), so more may come.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub count: Option<u32>,
    /// The crafting table to use, within reach; the bot's own 2x2 grid when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub table: Option<BlockPos>,
}

/// How many items `bot.craft` made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CraftResult {
    /// Items made: `count` or more, fewer only when the ingredients ran out after at least one craft.
    pub crafted: u32,
}

/// Smelts items in the furnace, blast furnace or smoker at `x, y, z` (within reach).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SmeltParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The item to smelt (`minecraft:raw_iron`, or just `raw_iron`).
    pub item: String,
    /// How many, 1 to 64; 1 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(min = 1, max = 64))]
    pub count: Option<u8>,
    /// Fuel to add (`coal`); none when absent, for a furnace that still burns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fuel: Option<String>,
    /// How much fuel; 1 when absent (one coal smelts 8 items).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(min = 1, max = 64))]
    pub fuel_count: Option<u8>,
}

/// How many items `bot.smelt` took out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SmeltResult {
    /// Items taken out of the output slot.
    pub smelted: u32,
}

/// The item to list recipes for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct RecipesForParams {
    pub bot: BotId,
    /// The item to make (`minecraft:stick`, or just `stick`).
    pub item: String,
}

/// Which grid `bot.craftableItems` counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CraftableItemsParams {
    pub bot: BotId,
    /// Count 3x3 recipes too, as at a crafting table; only the bot's own 2x2 grid when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub table: Option<bool>,
}

/// The items the inventory can craft now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CraftableItemsResult {
    /// One entry per item, in the server's recipe order, with what one craft of its first fitting recipe makes.
    pub items: Vec<ItemCount>,
}

/// The server's recipes for an item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct RecipesForResult {
    pub recipes: Vec<Recipe>,
}

/// One server recipe, its ingredients summed over the grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub id: String,
    /// Where it is made: `crafting_table` (a 2x2 recipe too), `furnace`, `stonecutter`, `smithing_table`, ...
    pub station: String,
    /// Fits the bot's own 2x2 grid (no crafting table needed).
    pub fits_2x2: bool,
    /// What one craft uses up.
    pub inputs: Vec<ItemCount>,
    /// What one craft makes.
    pub outputs: Vec<ItemCount>,
}

/// An item (or item tag) and a count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ItemCount {
    /// An item name, `#tag` for any item with that tag (`#minecraft:planks`), or a MoLang item expression.
    pub item: String,
    pub count: u32,
}
