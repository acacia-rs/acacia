//! Params and results of the container verbs. Each opens the block at `x, y, z` (within reach), acts and closes it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::BotId;

/// The container block cell `x, y, z` to read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ContainerParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// What a container held when it was read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ContainerContents {
    /// The non-empty slots, in slot order.
    pub items: Vec<ContainerItem>,
}

/// One non-empty container slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ContainerItem {
    pub slot: u32,
    /// The item's name, or `#<network id>` for one missing from the server's item table.
    pub item: String,
    pub count: u32,
}

/// Moves items of one kind between the container at `x, y, z` and the bot's main inventory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct TransferParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The item (`minecraft:cobblestone`, or just `cobblestone`).
    pub item: String,
    /// How many at most; all there are when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub count: Option<u32>,
}

/// How a `bot.deposit` or `bot.withdraw` went.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct TransferResult {
    /// Items moved: fewer than `count` once the source has no more or the other side is full.
    pub moved: u32,
}
