//! What a bot reports, mirroring acacia-bot's `BotEvent` (its docs say which group raises what).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BlockPos, Entity, EntityRow, Form, PlayerInfo, Stack, UniqueId};

/// One bot event. New variants are added within a protocol version: keep a catch-all arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum Event {
    /// In the world; comes right before `createBot` resolves.
    Spawn,
    /// Group `health`.
    Health { health: f32, food: f32, saturation: f32 },
    /// Group `health`.
    Death,
    /// Group `health`.
    Respawn,
    /// Group `chat`.
    Chat(ChatLine),
    /// A chat pattern matched (patterns run without the `chat` group).
    ChatMatch { pattern: String, groups: Vec<String>, line: ChatLine },
    /// Group `forms`.
    Form(Form),
    /// Group `players` (the list sent at spawn counts as joins).
    PlayerJoined(PlayerInfo),
    /// Group `players`.
    PlayerLeft(PlayerInfo),
    /// Group `entities`: an entity or other player came into view.
    EntitySpawn(Entity),
    /// Group `entities`: an entity left view or was removed (its last known state); all of them on a dimension change.
    EntityGone(Entity),
    /// Group `entityMoves`: every entity that moved since the last one (spawns count), once per tick batch, as
    /// rows; `bot.entities` has the rest of each.
    EntityMoves { rows: Vec<EntityRow> },
    /// Group `windows`: a window opened (the own inventory too); contents usually follow as `windowUpdate`.
    WindowOpen(Window),
    /// Group `windows`: the open window's contents arrived or were refreshed.
    WindowUpdate(Window),
    /// Group `windows`.
    #[serde(rename_all = "camelCase")]
    WindowClose { window_id: i32 },
    /// Group `windows`: the server opened a sign editor.
    SignEditor { position: BlockPos, front: bool },
    /// Group `titles`.
    Title { kind: TitleKind, text: String },
    /// Group `sleep`.
    Sleep,
    /// Group `sleep`.
    Wake,
    /// Group `ticks`: one or more client ticks ran.
    PhysicsTick,
    /// Strict mode only: a server packet a strict peer would reject.
    Violation { message: String },
    /// This many lossy events (see [`Event::is_lossy`]) were dropped because the consumer fell behind.
    Lagged { dropped: u64 },
    /// The host's task for the bot failed (a bug); its calls fail with `INTERNAL` and `end` follows.
    Error { message: String },
    /// The connection ended; always the bot's last event.
    End { reason: EndReason, message: String },
}

impl Event {
    /// A newer event or the state supersedes it, so a backed-up stream may drop it (and say so with `lagged`).
    pub fn is_lossy(&self) -> bool {
        matches!(self, Self::PhysicsTick | Self::Violation { .. } | Self::Title { .. } | Self::EntityMoves { .. })
    }
}

/// A chat, system or translated line the server sent (the `chat` event).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ChatLine {
    pub kind: ChatKind,
    /// The sender's name when the packet names one (`chat`, `whisper`, `announcement`), without `§` codes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sender: Option<String>,
    /// With `§` formatting codes; JSON (rawtext) messages are flattened to their text.
    pub message: String,
    /// `message` without formatting codes.
    pub plain: String,
    pub params: Vec<String>,
    /// The `{"rawtext":[...]}` JSON as sent, for a JSON message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub rawtext: Option<String>,
    /// The sender's XUID, when the server sends it (else empty).
    pub xuid: String,
}

/// The protocol's text type of a [`ChatLine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ChatKind {
    Chat,
    Whisper,
    Announcement,
    Raw,
    Translation,
    Popup,
    JukeboxPopup,
    Tip,
    System,
}

/// A window the server opened: a container block or entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub window_id: i32,
    /// The protocol's window type name (`Container`, `Furnace`, ...).
    pub window_type: String,
    /// The block that was opened, unless it is an entity container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub position: Option<BlockPos>,
    /// The entity that was opened (horse, chest minecart, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub entity: Option<UniqueId>,
    /// With `§` codes; absent for default names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    /// Non-empty slots only.
    pub slots: Vec<Stack>,
}

/// Where a `title` event's text shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TitleKind {
    Title,
    Subtitle,
    ActionBar,
    /// Removes the shown title (`text` is empty).
    Clear,
}

/// Why a bot ended (the `end` event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum EndReason {
    /// `bot.close`, or the host shutting down.
    Closed,
    Kicked,
    /// The server moved the player elsewhere; `message` holds the address.
    Transfer,
    LoginFailed,
    /// The connection broke (timeout, socket, protocol error).
    Lost,
}
