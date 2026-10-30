use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{EntityId, UniqueId, Vec3};

/// A tracked entity or other player (acacia-bot's `EntityInfo`). `kind`, not `type`: events are tagged by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    /// The runtime id actions take.
    pub id: EntityId,
    /// The id player lists and containers name.
    pub unique_id: UniqueId,
    /// `minecraft:zombie`; `minecraft:player` for players, `minecraft:item` for dropped items.
    pub kind: String,
    /// Players only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub username: Option<String>,
    /// Feet, players included.
    pub position: Vec3,
    /// The last velocity the server sent.
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// When the server sent it (mobs; players' health is not broadcast).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub health: Option<f32>,
}

/// Every tracked entity, as a bot last published them (read without a call).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct EntityList {
    /// Moves exactly when the list changes, so readers can cache on it.
    pub version: u64,
    pub entities: Vec<Entity>,
}
