//! Params of the host and bot-lifecycle methods.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BotId, CallId};

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
    /// Track entities and other players, needed for their positions, entity events, `follow` and `attack`
    /// (default false: on busy servers it is most of the decode work).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub entities: Option<bool>,
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

/// A group of events a bot can report (acacia-bot's `Events`). Opt-in and high-rate: `ticks` (`physicsTick`, 20 a
/// second) and `entityMoves` (`entityMoved`, once per tick batch); `entities` and `entityMoves` need `entities`.
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
    Entities,
    EntityMoves,
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
