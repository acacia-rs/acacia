//! Typed events derived from packets after the state has been updated. Each group is opt-in
//! ([`Events`]) so bots don't decode packets nobody reads (chat on a busy server, titles).
//!
//! Delivery, the one place it is written down:
//! - By default events queue on the bot and [`crate::Bot::next`] returns them, so what an action raises comes out
//!   after the action returns, and [`BotEvent::Tick`] comes only from `next` (ticks during an action are not kept).
//! - With [`crate::BotConfig::event_sink`] set, every event goes to the sink as it is produced, during actions too,
//!   ticks included: one call per event, in production order, on the bot's task. `next` then only drives the bot
//!   and returns `None` once the connection is gone; [`crate::Bot::wait_form`] cannot take its form back.
//! - The sink runs inside packet handling and must not block. [`channel`] is a bounded one: when full it drops the
//!   lossy events ([`BotEvent::is_lossy`]) and reports them as [`BotEvent::Lagged`], and keeps every other one.
//! - Either way, keep polling `next` between actions: auto-eat only runs while it does.

mod channel;
mod chat;
mod out;
mod source;

use std::ops::{BitOr, BitOrAssign};

use acacia_client::proto::packets::{SetTitle, SetTitleType};
use acacia_client::proto::RawPacket;
use acacia_client::{DisconnectReason, Violation};

pub use channel::{channel, EventReceiver};
pub use chat::{ChatKind, ChatMatch, ChatMessage, ChatPattern};
pub(crate) use chat::flatten as rawtext_value;
pub use out::EventSink;
pub(crate) use out::EventOut;
pub(crate) use source::EventSource;

use crate::forms::Form;
use crate::snapshot::EntityInfo;
use crate::state::{Container, PlayerListEntry};

/// Which groups of [`BotEvent`]s the bot emits (`Events::CHAT | Events::HEALTH`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Events(u32);

impl Events {
    pub const NONE: Self = Self(0);
    /// [`BotEvent::Chat`]. Chat patterns ([`crate::BotConfig::chat_patterns`]) run without it.
    pub const CHAT: Self = Self(1);
    /// [`BotEvent::Form`].
    pub const FORMS: Self = Self(1 << 1);
    /// [`BotEvent::Health`], [`BotEvent::Died`], [`BotEvent::Respawned`].
    pub const HEALTH: Self = Self(1 << 2);
    /// [`BotEvent::PlayerJoined`], [`BotEvent::PlayerLeft`] (the list sent at spawn counts as joins).
    pub const PLAYERS: Self = Self(1 << 3);
    /// [`BotEvent::WindowOpened`], [`BotEvent::WindowContents`], [`BotEvent::WindowClosed`], [`BotEvent::SignEditor`].
    pub const WINDOWS: Self = Self(1 << 4);
    /// [`BotEvent::Title`].
    pub const TITLES: Self = Self(1 << 5);
    /// [`BotEvent::Slept`], [`BotEvent::Woke`].
    pub const SLEEP: Self = Self(1 << 6);
    /// Every group above. Not the opt-in ones below.
    pub const ALL: Self = Self((1 << 7) - 1);
    /// [`BotEvent::Tick`]: wakes the caller 20 times a second.
    pub const TICKS: Self = Self(1 << 7);
    /// [`BotEvent::EntitySpawned`], [`BotEvent::EntityGone`]; needs `Trackers::entities`.
    pub const ENTITIES: Self = Self(1 << 8);
    /// [`BotEvent::EntitiesMoved`], once per tick batch; needs `Trackers::entities` and is high-rate on busy servers.
    pub const ENTITY_MOVES: Self = Self(1 << 9);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }
}

impl BitOr for Events {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Events {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum BotEvent {
    /// A packet the caller subscribed to, after state has been updated from it.
    Packet(RawPacket),
    Chat(ChatMessage),
    ChatMatch(ChatMatch),
    /// A server form waiting for [`crate::Bot::answer_form`].
    Form(Form),
    /// Health or hunger changed.
    Health { health: f32, food: f32, saturation: f32 },
    Died,
    Respawned,
    PlayerJoined(PlayerListEntry),
    PlayerLeft(PlayerListEntry),
    /// An entity or other player came into view.
    EntitySpawned(EntityInfo),
    /// An entity left view or was removed (last known state); every one on a dimension change.
    EntityGone(EntityInfo),
    /// Entities that moved since the last such event (spawns count).
    EntitiesMoved(Vec<EntityInfo>),
    /// A window opened (the own inventory too); its contents usually follow in later packets.
    WindowOpened(Container),
    /// The open window's contents arrived (`InventoryContent`): first after its [`BotEvent::WindowOpened`],
    /// then on every full refresh (live-updating GUIs resend them). Not for the own inventory.
    WindowContents(Container),
    WindowClosed { window_id: i32 },
    /// The server opened a sign editor ([`crate::Bot::write_sign`]).
    SignEditor { position: [i32; 3], front: bool },
    Title(Title),
    /// The server shows the player sleeping in a bed.
    Slept,
    Woke,
    /// One or more client ticks ran: the simulated player moved, for callers that draw it.
    Tick,
    /// Strict mode only ([`crate::BotConfig::strict`]).
    Violation(Violation),
    /// Only from [`channel`]: this many lossy events were dropped here because the consumer fell behind.
    Lagged { dropped: u64 },
    /// Always the last event.
    Disconnected(DisconnectReason),
}

impl BotEvent {
    /// A newer event or the state supersedes it (ticks, raw packets, violations, titles, moves), so a full queue
    /// may drop it.
    pub fn is_lossy(&self) -> bool {
        matches!(self, Self::Packet(_) | Self::Tick | Self::Violation(_) | Self::Title(_) | Self::EntitiesMoved(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleKind {
    Title,
    Subtitle,
    ActionBar,
    /// Removes the shown title (`text` is empty).
    Clear,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Title {
    pub kind: TitleKind,
    /// With `§` formatting codes; JSON titles are flattened to their text.
    pub text: String,
}

impl Title {
    fn from_packet(p: SetTitle) -> Option<Self> {
        use SetTitleType as T;
        let (kind, json) = match p.r#type {
            T::SetTitle => (TitleKind::Title, false),
            T::SetSubtitle => (TitleKind::Subtitle, false),
            T::ActionBarMessage => (TitleKind::ActionBar, false),
            T::SetTitleJson => (TitleKind::Title, true),
            T::SetSubtitleJson => (TitleKind::Subtitle, true),
            T::ActionBarMessageJson => (TitleKind::ActionBar, true),
            T::Clear | T::Reset => (TitleKind::Clear, false),
            T::SetDurations | T::Unknown(_) => return None,
        };
        let text = if json { chat::rawtext(&p.text) } else { p.text };
        Some(Self { kind, text })
    }
}

#[cfg(test)]
mod tests;
