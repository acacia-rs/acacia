//! Params of the movement and combat verbs: following, looking, attacking, held keys.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BotId, EntityId};

/// The entity to keep close to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct FollowParams {
    pub bot: BotId,
    /// A tracked entity's id (players included).
    pub entity: EntityId,
    /// Blocks (Euclidean) to keep within; 2 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub range: Option<f32>,
}

/// A world point the bot turns its eyes to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct LookAtParams {
    pub bot: BotId,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The entity to swing at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct AttackParams {
    pub bot: BotId,
    /// A tracked entity's id (players included).
    pub entity: EntityId,
}

/// A movement key to hold (`state` true) or release.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SetControlStateParams {
    pub bot: BotId,
    pub control: Control,
    pub state: bool,
}

/// A movement key (mineflayer's control names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Control {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sneak,
    Sprint,
}
