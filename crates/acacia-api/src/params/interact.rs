//! Params of the right-click and hand verbs: using blocks, items and entities, riding, the hotbar.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BotId, EntityId, Face, UniqueId};

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

/// What `bot.mount` seated the bot on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Vehicle {
    pub unique_id: UniqueId,
    /// The runtime id actions take; null when the vehicle's spawn was not seen.
    pub id: Option<EntityId>,
    /// `minecraft:boat`, ...; null when unknown.
    pub kind: Option<String>,
    /// In the controlling seat rather than a passenger seat.
    pub driver: bool,
}

/// The hotbar slot to hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SetQuickBarSlotParams {
    pub bot: BotId,
    /// The hotbar slot to select, 0-8.
    pub slot: u8,
}
