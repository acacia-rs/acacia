//! Params of the host and bot-lifecycle methods.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AutoEatPolicy, BotId, CallId};

/// The server to join and what the bot tracks there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBotParams {
    /// Server host name or address.
    pub host: String,
    /// Default `bedrock`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub edition: Option<Edition>,
    /// Default 19132 (Bedrock) or 25565 (Java).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub port: Option<u16>,
    /// The player name (`offline`), or the account's key in the token cache (`microsoft`: the server sees the
    /// Xbox gamertag, or the Java profile's name).
    pub username: String,
    /// Default `offline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auth: Option<Auth>,
    /// `microsoft` only: the token cache directory (see [`SignInParams::profiles_folder`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub profiles_folder: Option<String>,
    /// Bedrock only. Default `auto`: RakNet if the server answers a ping, else NetherNet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub transport: Option<Transport>,
    /// Simulate movement and track terrain, needed to walk (default false; Java bots always do).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub physics: Option<bool>,
    /// Track entities and other players, needed for their positions, entity events, `follow` and `attack`
    /// (default false: on busy servers it is most of the decode work).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub entities: Option<bool>,
    /// Press "respawn" about 2 s after dying, as a player reading the death screen would (default true; off, call
    /// `bot.respawn`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auto_respawn: Option<bool>,
    /// Eat when hungry, from the start (default off; `bot.setAutoEat` changes it). `{}` takes the defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auto_eat: Option<AutoEatPolicy>,
    /// Event groups reported from the start (default none; `bot.setEvents` changes them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub events: Option<Vec<EventGroup>>,
}

/// The game a server runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Edition {
    Bedrock,
    /// Java Edition 26.3.
    Java,
}

/// How a bot logs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Auth {
    /// No account: servers with `online-mode=false` (LAN, most test servers).
    Offline,
    /// A Microsoft account signed in by device code, its tokens cached per `username` (an `msaCode` record asks a
    /// person to sign in when the cache has none).
    Microsoft,
}

/// The Microsoft account to sign in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignInParams {
    /// The account's key in the token cache, as `createBot`'s `username`.
    pub username: String,
    /// Where tokens are cached, one `<username>.json` each (default `~/.acacia/auth` of the host's user). Holds
    /// refresh tokens: keep it private. acacia-daemon sets it itself and refuses this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub profiles_folder: Option<String>,
}

/// The Xbox profile a sign-in reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignInResult {
    /// The Xbox gamertag servers will see.
    pub display_name: String,
    pub xuid: String,
}

/// The network protocol a bot connects over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    Auto,
    Raknet,
    Nethernet,
}

/// A group of events a bot can report (acacia-bot's `Events`). Opt-in and high-rate: `ticks` (`physicsTick`, 20 a
/// second) and `entityMoves` (once per tick batch); `entities` and `entityMoves` need `entities`.
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

/// The new bot's id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CreateBotResult {
    pub bot: BotId,
}

/// The bot a method without other params acts on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct BotParams {
    pub bot: BotId,
}

/// A bot's call to cancel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CancelParams {
    pub bot: BotId,
    /// The call to cancel.
    pub call: CallId,
}

/// A host call (no bot yet) to cancel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct CancelCallParams {
    /// The `createBot` or `signIn` call to cancel.
    pub call: CallId,
}

/// The event groups the bot reports from now on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct SetEventsParams {
    pub bot: BotId,
    pub events: Vec<EventGroup>,
}
