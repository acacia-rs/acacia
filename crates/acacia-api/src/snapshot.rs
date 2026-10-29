//! Plain copies of a bot's state, mirroring acacia-bot's `snapshot` module (its docs hold the hot/cold contract).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{UniqueId, Vec3};

/// The bot's state at one moment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Snapshot {
    pub hot: HotSnapshot,
    pub cold: ColdSnapshot,
}

/// Changes nearly every tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HotSnapshot {
    /// Client ticks since the bot connected (0 when the bot does not publish snapshots).
    pub tick: u64,
    pub position: Vec3,
    /// Physics bots only (zero otherwise), like `onGround` and `gliding`.
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub health: f32,
    pub max_health: f32,
    pub food: f32,
    pub saturation: f32,
    pub xp_level: i32,
    pub xp_progress: f32,
    pub air: i16,
    pub alive: bool,
    /// The protocol's game mode number (0 survival, 1 creative, 2 adventure, 6 spectator).
    pub game_mode: i32,
    pub dimension: i32,
    pub held_slot: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub open_window: Option<i32>,
    pub flying: bool,
    pub sleeping: bool,
    pub riding: bool,
    pub gliding: bool,
}

/// Changes on inventory, effect and player-list updates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ColdSnapshot {
    /// Moves exactly when the cold part is rebuilt (0 in an on-demand snapshot), so readers can cache on it.
    pub version: u64,
    pub inventory: Vec<Stack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cursor: Option<Stack>,
    pub effects: Vec<Effect>,
    pub players: Vec<PlayerInfo>,
}

/// A non-empty stack, summarised. `networkId` is the server's item id; names come from its item table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub window: i32,
    pub slot: u8,
    pub network_id: i32,
    pub count: u16,
    pub metadata: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stack_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Effect {
    pub id: i32,
    pub amplifier: i32,
    /// Ticks left.
    pub duration: i32,
}

/// A player-list entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
    pub uuid: String,
    pub username: String,
    /// Empty for offline players.
    pub xuid: String,
    pub unique_id: UniqueId,
}
