//! The method table: the one list of what a transport can call. Add a method here and the generated
//! `methods.json` / `methods.ts` follow (`tests/generated.rs`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::table::{api, Target};
use crate::{BotId, CallId, FormAnswer, Snapshot};

api! {
    /// Connects a bot and resolves once it has spawned. The `done` record names the new bot.
    "createBot" => CreateBot(CreateBotParams) -> CreateBotResult, Host [];
    /// Disconnects the bot (waiting up to 2 s for the server to free the session). The running call is
    /// cancelled and queued ones fail with `DISCONNECTED`; an `end` event comes before this resolves.
    "bot.close" => Close(BotParams) -> (), Immediate [];
    /// Cancels call `call`: removed if still queued, ended within a tick if running. Fails with `CANCELLED`.
    "cancel" => Cancel(CancelParams) -> (), Immediate [];
    /// The bot's state now, built on demand.
    "bot.snapshot" => Snapshot(BotParams) -> Snapshot, Immediate [mcp];
    /// Changes the event groups the bot reports.
    "bot.setEvents" => SetEvents(SetEventsParams) -> (), Immediate [];
    /// Sends a chat line.
    "bot.chat" => Chat(ChatParams) -> (), Immediate [mcp];
    /// Runs a slash command (the leading `/` is optional).
    "bot.command" => Command(CommandParams) -> (), Immediate [mcp];
    /// Walks to a goal (physics bots only). Fails with `NOT_POSSIBLE` when there is no path.
    "bot.goto" => Goto(GotoParams) -> (), Queued [mcp];
    /// Breaks the block at a cell, as a player would (reach, tool, mining time).
    "bot.dig" => Dig(DigParams) -> (), Queued [mcp];
    /// Answers an open form after a human reading delay.
    "bot.answerForm" => AnswerForm(AnswerFormParams) -> (), Queued [mcp];
    /// Lets ticks pass (50 ms each) while the bot keeps playing.
    "bot.waitForTicks" => WaitForTicks(WaitForTicksParams) -> (), Queued [mcp];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBotParams {
    /// Server host name or address.
    pub host: String,
    /// Default 19132.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub port: Option<u16>,
    pub username: String,
    /// Default `offline`, the only one so far.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auth: Option<Auth>,
    /// Default `auto`: RakNet if the server answers a ping, else NetherNet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub transport: Option<Transport>,
    /// Simulate movement and track terrain, needed to walk (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub physics: Option<bool>,
    /// Event groups reported from the start (default none; `bot.setEvents` changes them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub events: Option<Vec<EventGroup>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Auth {
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    Auto,
    Raknet,
    Nethernet,
}

/// A group of events a bot can report (acacia-bot's `Events`); `ticks` is `physicsTick`, 20 a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EventGroup {
    Chat,
    Forms,
    Health,
    Players,
    Windows,
    Titles,
    Sleep,
    Ticks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CreateBotResult {
    pub bot: BotId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct BotParams {
    pub bot: BotId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CancelParams {
    pub bot: BotId,
    /// The call to cancel.
    pub call: CallId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SetEventsParams {
    pub bot: BotId,
    pub events: Vec<EventGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ChatParams {
    pub bot: BotId,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CommandParams {
    pub bot: BotId,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct DigParams {
    pub bot: BotId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
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
    pub ticks: u32,
}

impl Target for CreateBotParams {
    fn bot(&self) -> Option<&BotId> {
        None
    }
}

macro_rules! bot_params {
    ($($t:ty),*) => {$(
        impl Target for $t {
            fn bot(&self) -> Option<&BotId> {
                Some(&self.bot)
            }
        }
    )*};
}

bot_params!(BotParams, CancelParams, SetEventsParams, ChatParams, CommandParams, GotoParams, DigParams, AnswerFormParams, WaitForTicksParams);
