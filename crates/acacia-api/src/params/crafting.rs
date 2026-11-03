//! Params and results of crafting.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BlockPos, BotId};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CraftResult {
    /// Items made: `count` or more, fewer only when the ingredients ran out after at least one craft.
    pub crafted: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct RecipesForParams {
    pub bot: BotId,
    /// The item to make (`minecraft:stick`, or just `stick`).
    pub item: String,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ItemCount {
    /// An item name, `#tag` for any item with that tag (`#minecraft:planks`), or a MoLang item expression.
    pub item: String,
    pub count: u32,
}
