//! Params and results of the workstation verbs: anvil, enchanting table, grindstone, smithing table, stonecutter,
//! loom, cartography table, beacon, brewing stand and trades. Each block verb opens the block at `x, y, z` (within
//! reach), acts, takes what it made into the inventory and closes it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BotId, EntityId, SlotRef};

/// Repairs, combines and/or renames at an anvil; needs a `material`, a `rename` or both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct AnvilParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Into the first slot.
    pub input: SlotRef,
    /// Into the second slot: a repair material, or a second item or book to combine with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub material: Option<SlotRef>,
    /// The result's new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rename: Option<String>,
}

/// Enchants an item at an enchanting table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct EnchantParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub item: SlotRef,
    /// The offered enchantment, 0-2 (cheapest first); needs its level and `option + 1` lapis lazuli.
    #[schemars(range(max = 2))]
    pub option: u8,
}

/// Disenchants an item at a grindstone, or merges it with a second one of the same kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct GrindstoneParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub input: SlotRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub additional: Option<SlotRef>,
}

/// Upgrades or trims an item at a smithing table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SmithParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The item to upgrade or trim.
    pub base: SlotRef,
    /// The ingot or trim material.
    pub addition: SlotRef,
    /// The smithing template.
    pub template: SlotRef,
}

/// Cuts an item at a stonecutter from what the inventory holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct StonecutParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The item to make (`minecraft:stone_bricks`, or just `stone_bricks`).
    pub item: String,
    /// At least this many; 1 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub count: Option<u32>,
}

/// How many items `bot.stonecut` made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct StonecutResult {
    /// Fewer than asked only when the input ran out after the first cut.
    pub made: u32,
}

/// Adds a pattern to a banner at a loom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoomParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub banner: SlotRef,
    /// The pattern's colour.
    pub dye: SlotRef,
    /// The Bedrock pattern id (`bo`, `cr`, `flo`, ...).
    pub pattern: String,
    /// The banner pattern item the design needs, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pattern_item: Option<SlotRef>,
}

/// Clones, zooms out, locks and/or renames filled maps at a cartography table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CartographyParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// A filled map stack.
    pub map: SlotRef,
    /// An empty map clones, paper zooms out, a glass pane locks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub additional: Option<SlotRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rename: Option<String>,
    /// Crafts to make (a clone makes two maps, a rename one); 1 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub times: Option<u32>,
}

/// How many crafts `bot.cartography` made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CartographyResult {
    /// The crafts that went through.
    pub crafted: u32,
}

/// A beacon power.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BeaconEffect {
    Speed,
    Haste,
    Strength,
    JumpBoost,
    Regeneration,
    Resistance,
}

/// Sets a beacon's powers, paying one item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct BeaconParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub primary: BeaconEffect,
    /// Needs a full pyramid: `regeneration`, or the primary again for level II.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub secondary: Option<BeaconEffect>,
    /// An iron, gold or netherite ingot, an emerald or a diamond.
    pub payment: SlotRef,
}

/// Brews at a brewing stand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct BrewParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// One to three bottle slots.
    #[schemars(length(min = 1, max = 3))]
    pub bottles: Vec<SlotRef>,
    /// The ingredient (`minecraft:nether_wart`, or just `nether_wart`).
    pub ingredient: String,
}

/// Trades with a villager or wandering trader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct TradeParams {
    pub bot: BotId,
    /// A tracked villager's or wandering trader's id.
    pub entity: EntityId,
    /// The offer's index in the trade screen's list, from 0.
    pub offer: u32,
    /// 1 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub times: Option<u32>,
}

/// How many trades `bot.trade` made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct TradeResult {
    /// Fewer than asked only when payment or stock ran out after one trade.
    pub traded: u32,
}
