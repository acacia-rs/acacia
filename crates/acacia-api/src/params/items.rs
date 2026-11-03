//! Params and results of the inventory and block verbs: equipping, placing, finding blocks, eating, tossing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BlockPos, BotId, Vec3};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct EquipParams {
    pub bot: BotId,
    /// An item name (`minecraft:stone`, or just `stone`) in the main inventory.
    pub item: String,
    /// `hand` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub destination: Option<EquipDestination>,
}

/// Where `bot.equip` puts an item (mineflayer's names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EquipDestination {
    /// Into the hotbar, selected.
    Hand,
    Head,
    Torso,
    Legs,
    Feet,
    OffHand,
}

/// Places the held block against `face` of the block at `x, y, z`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct PlaceBlockParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `up` when absent: on top of the block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub face: Option<Face>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindBlocksParams {
    pub bot: BotId,
    /// Block names (`minecraft:oak_log` or `oak_log`), any state.
    pub matching: Vec<String>,
    /// Blocks from the bot's feet (Euclidean); 16 when absent, at most 64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_distance: Option<u32>,
    /// How many to return, nearest first; 1 when absent, at most 1024.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct FindBlocksResult {
    /// Nearest first.
    pub blocks: Vec<BlockPos>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ConsumeParams {
    pub bot: BotId,
    /// A food or drink to take into the hand first; the held item when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub item: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct TossParams {
    pub bot: BotId,
    /// The item (`minecraft:dirt`, or just `dirt`).
    pub item: String,
    /// How many at most; all the inventory has when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub count: Option<u32>,
    /// A point to look at first, so the items fly that way (a player's eyes); where the bot looks when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub toward: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct TossResult {
    pub tossed: u32,
}
