//! Params of chat, movement and world actions. Positions are flat `x, y, z` fields: MCP clients handle flat
//! params best.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BotId, FormAnswer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ChatParams {
    pub bot: BotId,
    /// The line everyone sees.
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CommandParams {
    pub bot: BotId,
    /// The command line, e.g. `/time set day`.
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct GotoParams {
    pub bot: BotId,
    pub goal: Goal,
}

/// Where `bot.goto` walks to; `range` is Euclidean, in blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Goal {
    /// Feet in this cell.
    Block { x: i32, y: i32, z: i32 },
    /// Within `range` of the cell.
    Near { x: i32, y: i32, z: i32, range: f32 },
    /// Any height in this column.
    Xz { x: i32, z: i32 },
    /// Feet at this height.
    Y { y: i32 },
    /// At least `range` from the cell: fleeing it.
    Away { x: i32, y: i32, z: i32, range: f32 },
    /// Whichever goal is reached first.
    Any { goals: Vec<Goal> },
}

/// The block cell `x, y, z` to break.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DigParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Hold the fastest tool for the block first, as `bot.equipBestTool` does (default false: the held item).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub equip_best_tool: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct AnswerFormParams {
    pub bot: BotId,
    /// The form's id, from its `form` event.
    pub form: u32,
    pub answer: FormAnswer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct WaitForTicksParams {
    pub bot: BotId,
    /// 20 a second.
    pub ticks: u32,
}
