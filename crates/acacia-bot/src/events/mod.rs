//! Typed events derived from packets after the state has been updated. Each group is opt-in
//! ([`Events`]) so bots don't decode packets nobody reads (chat on a busy server, titles).

mod chat;

use std::collections::VecDeque;
use std::ops::{BitOr, BitOrAssign};

use acacia_client::proto::packets::{ModalFormRequest, OpenSign, SetTitle, SetTitleType, Text};
use acacia_client::proto::{Packet, RawPacket};
use acacia_client::{DisconnectReason, Violation};

pub use chat::{ChatKind, ChatMatch, ChatMessage, ChatPattern};
pub(crate) use chat::flatten as rawtext_value;

use crate::forms::Form;
use crate::state::{Container, GameState, PlayerChange, PlayerListEntry};

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
    /// [`BotEvent::WindowOpened`], [`BotEvent::WindowClosed`], [`BotEvent::SignEditor`].
    pub const WINDOWS: Self = Self(1 << 4);
    /// [`BotEvent::Title`].
    pub const TITLES: Self = Self(1 << 5);
    /// [`BotEvent::Slept`], [`BotEvent::Woke`].
    pub const SLEEP: Self = Self(1 << 6);
    /// Every group above. Not [`Events::TICKS`], which wakes the caller 20 times a second.
    pub const ALL: Self = Self((1 << 7) - 1);
    /// [`BotEvent::Tick`].
    pub const TICKS: Self = Self(1 << 7);

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
    /// A window opened (the own inventory too); its contents usually follow in later packets.
    WindowOpened(Container),
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
    /// Always the last event.
    Disconnected(DisconnectReason),
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

/// Turns applied packets into [`BotEvent`]s, remembering what it last reported.
pub(crate) struct EventSource {
    events: Events,
    patterns: Vec<ChatPattern>,
    vitals: (f32, f32, f32),
    alive: bool,
    window: Option<i32>,
    sleeping: bool,
}

impl EventSource {
    pub(crate) fn new(events: Events, patterns: Vec<ChatPattern>) -> Self {
        Self { events, patterns, vitals: (20.0, 20.0, 5.0), alive: true, window: None, sleeping: false }
    }

    /// Packets the enabled events need beyond the always-on trackers.
    pub(crate) fn packet_ids(&self) -> Vec<u32> {
        let mut ids = Vec::new();
        if self.events.contains(Events::CHAT) || !self.patterns.is_empty() {
            ids.push(Text::ID);
        }
        if self.events.contains(Events::TITLES) {
            ids.push(SetTitle::ID);
        }
        ids
    }

    pub(crate) fn wants(&self, group: Events) -> bool {
        self.events.contains(group)
    }

    /// Queues the events `packet` caused; `state` already reflects it.
    pub(crate) fn on_packet(&mut self, packet: &RawPacket, state: &mut GameState, out: &mut VecDeque<BotEvent>) {
        let changes = std::mem::take(&mut state.player_list.changes);
        if self.wants(Events::PLAYERS) {
            out.extend(changes.into_iter().map(|c| match c {
                PlayerChange::Joined(p) => BotEvent::PlayerJoined(p),
                PlayerChange::Left(p) => BotEvent::PlayerLeft(p),
            }));
        }
        match packet.id {
            // Text also arrives for bed refusals (sleep.rs) when chat is off.
            Text::ID if self.wants(Events::CHAT) || !self.patterns.is_empty() => self.on_text(packet, out),
            OpenSign::ID if self.wants(Events::WINDOWS) => {
                if let Some(e) = state.signs.editor {
                    out.push_back(BotEvent::SignEditor { position: e.position, front: e.front });
                }
            }
            ModalFormRequest::ID if self.wants(Events::FORMS) => {
                let id = packet.decode::<ModalFormRequest>().map(|p| p.form_id);
                if let Some(form) = id.ok().and_then(|id| state.forms.get(id)) {
                    out.push_back(BotEvent::Form(form.clone()));
                }
            }
            SetTitle::ID if self.wants(Events::TITLES) => {
                if let Some(title) = packet.decode().ok().and_then(Title::from_packet) {
                    out.push_back(BotEvent::Title(title));
                }
            }
            _ => {}
        }
        if self.wants(Events::HEALTH) {
            self.on_vitals(state, out);
        }
        if self.wants(Events::WINDOWS) {
            self.on_window(state, out);
        }
        if self.wants(Events::SLEEP) && state.player.is_sleeping() != self.sleeping {
            self.sleeping = !self.sleeping;
            out.push_back(if self.sleeping { BotEvent::Slept } else { BotEvent::Woke });
        }
    }

    fn on_text(&self, packet: &RawPacket, out: &mut VecDeque<BotEvent>) {
        let Some(message) = packet.decode::<Text>().ok().and_then(ChatMessage::from_packet) else { return };
        if !self.patterns.is_empty() {
            let plain = message.plain();
            for pattern in &self.patterns {
                if let Some(groups) = pattern.captures(&plain) {
                    out.push_back(BotEvent::ChatMatch(ChatMatch { pattern: pattern.name.clone(), groups, message: message.clone() }));
                }
            }
        }
        if self.wants(Events::CHAT) {
            out.push_back(BotEvent::Chat(message));
        }
    }

    fn on_vitals(&mut self, state: &GameState, out: &mut VecDeque<BotEvent>) {
        let p = &state.player;
        let vitals = (p.health, p.hunger, p.saturation);
        if vitals != self.vitals {
            self.vitals = vitals;
            out.push_back(BotEvent::Health { health: p.health, food: p.hunger, saturation: p.saturation });
        }
        if p.alive != self.alive {
            self.alive = p.alive;
            out.push_back(if p.alive { BotEvent::Respawned } else { BotEvent::Died });
        }
    }

    fn on_window(&mut self, state: &GameState, out: &mut VecDeque<BotEvent>) {
        let open = state.containers.open.as_ref();
        let id = open.map(|c| c.window_id);
        if id == self.window {
            return;
        }
        if let Some(window_id) = self.window {
            out.push_back(BotEvent::WindowClosed { window_id });
        }
        if let Some(container) = open {
            out.push_back(BotEvent::WindowOpened(container.clone()));
        }
        self.window = id;
    }
}

#[cfg(test)]
mod tests;
