//! Params and results of the survival verbs: tools, auto-eat, beds and signs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::BotId;

/// The block cell `x, y, z` to pick the tool for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct EquipBestToolParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// What `bot.equipBestTool` holds now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct EquipBestToolResult {
    /// The hotbar slot now held; null when nothing in the inventory beats the bare hand (the held item is kept).
    pub slot: Option<u8>,
}

/// How auto-eat picks its moment and its food; every field has a default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AutoEatPolicy {
    /// Eat when the hunger bar (0-20) drops below this; 14 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(range(min = 1, max = 20))]
    pub below: Option<f32>,
    /// Which food to pick; `saturation` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prefer: Option<FoodPreference>,
    /// Move food into the hotbar from the main inventory when the hotbar has none; true when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub from_inventory: Option<bool>,
}

/// Which food auto-eat picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FoodPreference {
    /// The most saturation.
    Saturation,
    /// The most hunger points that still fit in the bar, else the fewest.
    NoWaste,
}

/// Auto-eat on (with its policy) or off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SetAutoEatParams {
    pub bot: BotId,
    /// On or off.
    pub enabled: bool,
    #[serde(flatten)]
    #[ts(flatten)]
    pub policy: AutoEatPolicy,
}

/// The bed at `x, y, z` (either half).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SleepParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Writes one side of the sign at `x, y, z` (within reach).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct WriteSignParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Up to 4 lines, 256 bytes in all.
    #[schemars(length(max = 4))]
    pub lines: Vec<String>,
    /// The back side; the front when absent. The server opens the side the bot faces, so stand on that side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub back: Option<bool>,
}
