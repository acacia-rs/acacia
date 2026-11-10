use acacia_client::proto::packets::{ModalFormRequest, OpenSign, SetTitle, Text};
use acacia_client::proto::{Packet, RawPacket};
use acacia_client::PacketFilter;

use super::{BotEvent, ChatMatch, ChatMessage, ChatPattern, EventOut, Events, Title};
use crate::snapshot::EntityInfo;
use crate::state::{EntityChange, GameState, PlayerChange};

/// Turns applied packets into [`BotEvent`]s, remembering what it last reported.
pub(crate) struct EventSource {
    events: Events,
    patterns: Vec<ChatPattern>,
    /// The connection's packet filter without the packets only events need.
    base: PacketFilter,
    vitals: (f32, f32, f32),
    alive: bool,
    window: Option<i32>,
    contents: u64,
    sleeping: bool,
    /// The entity clock ([`crate::state::Entities::now`]) at the last [`BotEvent::EntitiesMoved`].
    moves_seen: u64,
}

impl EventSource {
    pub(crate) fn new(events: Events, patterns: Vec<ChatPattern>) -> Self {
        let base = PacketFilter::none();
        let (vitals, window, moves_seen) = ((20.0, 20.0, 5.0), None, 0);
        Self { events, patterns, base, vitals, alive: true, window, contents: 0, sleeping: false, moves_seen }
    }

    /// `base`: the packets the connection takes whatever the events.
    pub(crate) fn with_base(self, base: PacketFilter) -> Self {
        Self { base, ..self }
    }

    pub(crate) fn events(&self) -> Events {
        self.events
    }

    pub(crate) fn set(&mut self, events: Events) {
        self.events = events;
    }

    /// The connection's packet filter for the enabled events.
    pub(crate) fn filter(&self) -> PacketFilter {
        self.packet_ids().into_iter().fold(self.base.clone(), PacketFilter::with)
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
    pub(crate) fn on_packet(&mut self, packet: &RawPacket, state: &mut GameState, out: &mut EventOut) {
        let changes = std::mem::take(&mut state.player_list.changes);
        if self.wants(Events::PLAYERS) {
            out.extend(changes.into_iter().map(|c| match c {
                PlayerChange::Joined(p) => BotEvent::PlayerJoined(p),
                PlayerChange::Left(p) => BotEvent::PlayerLeft(p),
            }));
        }
        self.on_entity_changes(state, out);
        match packet.id {
            // Text also arrives for bed refusals (sleep.rs) when chat is off.
            Text::ID if self.wants(Events::CHAT) || !self.patterns.is_empty() => self.on_text(packet, out),
            OpenSign::ID if self.wants(Events::WINDOWS) => {
                if let Some(e) = state.signs.editor {
                    out.push(BotEvent::SignEditor { position: e.position, front: e.front });
                }
            }
            ModalFormRequest::ID if self.wants(Events::FORMS) => {
                let id = packet.decode::<ModalFormRequest>().map(|p| p.form_id);
                if let Some(form) = id.ok().and_then(|id| state.forms.get(id)) {
                    out.push(BotEvent::Form(form.clone()));
                }
            }
            SetTitle::ID if self.wants(Events::TITLES) => {
                if let Some(title) = packet.decode().ok().and_then(Title::from_packet) {
                    out.push(BotEvent::Title(title));
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
            out.push(if self.sleeping { BotEvent::Slept } else { BotEvent::Woke });
        }
    }

    /// After the ticks that came due: one [`BotEvent::EntitiesMoved`] listing every entity that moved since the last.
    pub(crate) fn on_ticks(&mut self, state: &GameState, out: &mut EventOut) {
        let now = state.entities.now();
        if !self.wants(Events::ENTITY_MOVES) {
            self.moves_seen = now;
            return;
        }
        let seen = std::mem::replace(&mut self.moves_seen, now);
        let moved: Vec<_> = state.entities.iter().filter(|e| e.moved_at >= seen).map(EntityInfo::copy).collect();
        if !moved.is_empty() {
            out.push(BotEvent::EntitiesMoved(moved));
        }
    }

    fn on_entity_changes(&self, state: &mut GameState, out: &mut EventOut) {
        if state.entities.changes.is_empty() {
            return;
        }
        let changes = std::mem::take(&mut state.entities.changes);
        if !self.wants(Events::ENTITIES) {
            return;
        }
        for change in changes {
            match change {
                EntityChange::Spawned(id) => {
                    if let Some(e) = state.entities.get(id) {
                        out.push(BotEvent::EntitySpawned(EntityInfo::copy(e)));
                    }
                }
                EntityChange::Gone(e) => out.push(BotEvent::EntityGone(EntityInfo::copy(&e))),
            }
        }
    }

    fn on_text(&self, packet: &RawPacket, out: &mut EventOut) {
        let Some(message) = packet.decode::<Text>().ok().and_then(ChatMessage::from_packet) else { return };
        if !self.patterns.is_empty() {
            let plain = message.plain();
            for pattern in &self.patterns {
                if let Some(groups) = pattern.captures(&plain) {
                    out.push(BotEvent::ChatMatch(ChatMatch { pattern: pattern.name.clone(), groups, message: message.clone() }));
                }
            }
        }
        if self.wants(Events::CHAT) {
            out.push(BotEvent::Chat(message));
        }
    }

    fn on_vitals(&mut self, state: &GameState, out: &mut EventOut) {
        let p = &state.player;
        let vitals = (p.health, p.hunger, p.saturation);
        if vitals != self.vitals {
            self.vitals = vitals;
            out.push(BotEvent::Health { health: p.health, food: p.hunger, saturation: p.saturation });
        }
        if p.alive != self.alive {
            self.alive = p.alive;
            out.push(if p.alive { BotEvent::Respawned } else { BotEvent::Died });
        }
    }

    fn on_window(&mut self, state: &GameState, out: &mut EventOut) {
        let open = state.containers.open.as_ref();
        let id = open.map(|c| c.window_id);
        if id != self.window {
            if let Some(window_id) = self.window {
                out.push(BotEvent::WindowClosed { window_id });
            }
            if let Some(container) = open {
                out.push(BotEvent::WindowOpened(container.clone()));
            }
            self.window = id;
        }
        if state.containers.contents != self.contents {
            self.contents = state.containers.contents;
            if let Some(container) = open {
                out.push(BotEvent::WindowContents(container.clone()));
            }
        }
    }
}
