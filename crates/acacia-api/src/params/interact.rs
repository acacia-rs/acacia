//! Params of the right-click and hand verbs: using blocks, items and entities, the hotbar.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BotId, EntityId, Face};

/// The block cell `x, y, z` to right-click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ActivateBlockParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The face to click when the bot sees it, else one it sees; `up` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub face: Option<Face>,
}

/// The entity to right-click.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ActivateEntityParams {
    pub bot: BotId,
    /// A tracked entity's id (players included).
    pub entity: EntityId,
}

/// The hotbar slot to hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SetQuickBarSlotParams {
    pub bot: BotId,
    /// The hotbar slot to select, 0-8.
    pub slot: u8,
}
