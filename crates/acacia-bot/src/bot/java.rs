//! The Java Edition side of a [`Bot`] (docs/java-edition.md, build step 4): it joins, ticks and walks by
//! [`Controls`]; actions that need the Bedrock wire return [`crate::ActionError::Unsupported`].

use acacia_client::proto::types::Vec3f;
use acacia_client::DisconnectReason;
use acacia_java_client::{Client, Error, Packet};
use acacia_java_physics::{wrap_degrees, Keys};
use acacia_java_proto::ids::play::clientbound::{LOGIN, PLAYER_POSITION, RESPAWN, SET_PASSENGERS};

use super::bedrock::Backend;
use super::java_setbacks::Setbacks;
use super::Step;
use super::Assembly;
use crate::events::{EventOut, EventSource};
use crate::movement::{java_keys, Controls, JavaSim, Live};
use crate::state::GameState;
use crate::{Bot, BotConfig};

/// A Java connection; its client applies the world and movement packets itself.
pub struct Java {
    pub(crate) client: Client,
    pub(crate) controls: Controls,
    /// Sequence of the predicting action awaiting the server's answer, 0 for none.
    pub(crate) awaiting: i32,
    /// Keeps the own inventory open between item moves, as crafting in its grid needs: closing it hands the grid
    /// back.
    pub(crate) inventory_held: bool,
    ticks: u64,
    setbacks: Setbacks,
}

impl Java {
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Server positions that corrected the player ([`Setbacks`] says which don't).
    pub fn setbacks(&self) -> u32 {
        self.setbacks.count
    }

    pub(crate) fn started(&self) -> bool {
        self.client.loaded()
    }

    /// The server has sent the player's position: it ticks from login, before that.
    pub(crate) fn placed(&self) -> bool {
        self.setbacks.teleports > 0
    }
}

/// Every server position after the first is a setback.
impl Live for Java {
    type Sim = JavaSim;

    fn base(&self) -> Option<JavaSim> {
        let mut player = self.client.movement.player.clone();
        player.keys = Keys::default();
        self.started().then_some(JavaSim(player))
    }

    fn fork(&self) -> Option<JavaSim> {
        self.started().then(|| JavaSim(self.client.movement.player.clone()))
    }

    fn on_ground(&self) -> bool {
        self.client.movement.player.on_ground
    }

    fn teleports(&self) -> u32 {
        self.setbacks.teleports
    }

    fn setback_age(&self) -> Option<u64> {
        self.setbacks.last.map(|t| self.ticks.saturating_sub(t))
    }

    fn controls(&mut self) -> &mut Controls {
        &mut self.controls
    }
}

impl Bot {
    /// Joins an offline-mode Java server as `name`. Only `config`'s edition-neutral settings apply.
    pub async fn connect_java_offline(host: &str, port: u16, name: &str, config: BotConfig) -> Result<Bot, Error> {
        Ok(Self::java_bot(Client::join_offline(host, port, name).await?, config))
    }

    /// Joins any Java server as `account`'s Java profile. Only `config`'s edition-neutral settings apply.
    pub async fn connect_java(host: &str, port: u16, account: &acacia_client::auth::Account, config: BotConfig) -> Result<Bot, Error> {
        let client = Client::join_online(host, port, account).await?;
        if let Some(e) = client.profile_key_error() {
            tracing::warn!(error = %e, "no profile key: chat goes out unsigned, which secure-chat servers refuse");
        }
        Ok(Self::java_bot(client, config))
    }

    fn java_bot(client: Client, config: BotConfig) -> Bot {
        let events = EventSource::new(config.events, config.chat_patterns);
        let mut state = GameState::new(config.trackers, 0);
        state.items = crate::state::ItemRegistry::java();
        state.recipes = crate::state::Recipes::java();
        let java = Java { client, controls: Controls::default(), awaiting: 0, inventory_held: false, ticks: 0, setbacks: Setbacks::default() };
        let assembly = Assembly { auto_respawn: config.auto_respawn, auto_eat: config.auto_eat, snapshots: config.snapshots };
        Bot::assemble(Backend::Java(Box::new(java)), state, events, EventOut::new(config.event_sink), assembly)
    }

    /// The Java side, for Java bots.
    pub fn java(&self) -> Option<&Java> {
        match &self.backend {
            Backend::Java(java) => Some(java),
            Backend::Bedrock(_) => None,
        }
    }

    /// [`Bot::step`] on Java.
    pub(super) async fn java_step(&mut self) -> Option<Step> {
        if self.closed.is_some() {
            return None;
        }
        // Vanilla handles every queued packet before a frame's ticks. Grim's transactions rely on it: it writes a ping
        // before the packet it times, so a tick between the two runs a move Grim judges with the packet applied.
        let Backend::Java(java) = &mut self.backend else { return None };
        if let Some(result) = java.client.try_next() {
            return Some(self.on_java_packet(result));
        }
        if self.ticker.due() {
            // Deferred until the I/O driver has run, so this shard's readers decode what already arrived.
            tokio::task::yield_now().await;
            let Backend::Java(java) = &mut self.backend else { return None };
            if let Some(result) = java.client.try_next() {
                return Some(self.on_java_packet(result));
            }
        }
        let Backend::Java(java) = &mut self.backend else { return None };
        tokio::select! {
            biased;
            result = java.client.next() => Some(self.on_java_packet(result)),
            Some(request) = self.inbox.rx.recv() => {
                self.on_immediate(request);
                Some(Step::Idle)
            }
            ticks = self.ticker.wait() => {
                self.run_ticks(ticks);
                Some(Step::Tick)
            }
        }
    }

    fn on_java_packet(&mut self, result: Result<Packet, Error>) -> Step {
        match result {
            Ok(packet) => {
                // A teleport turns the player too; the controls' look follows it, as the Bedrock spawn does.
                if packet.id == PLAYER_POSITION
                    && let Backend::Java(java) = &mut self.backend
                {
                    let player = &java.client.movement.player;
                    (java.controls.yaw, java.controls.pitch) = (player.yaw, player.pitch);
                    // Snapshots read the body only once the client has loaded; until then they read this.
                    let [x, y, z] = player.pos.map(|c| c as f32);
                    let mirrored = &mut self.state.player;
                    (mirrored.position, mirrored.yaw, mirrored.pitch) = (Vec3f { x, y, z }, player.yaw, player.pitch);
                    let teleport = <acacia_java_proto::packets::PlayerPosition as acacia_java_proto::Wire>::read(&mut &packet.body[..]).map(|p| p.id);
                    tracing::trace!(tick = java.ticks, ?teleport, "java teleport");
                    java.setbacks.in_bed |= java.client.at_sleep_spot();
                    if java.setbacks.on_position(java.ticks) {
                        tracing::debug!(tick = java.ticks, pos = ?player.pos, loaded = java.client.loaded(), "java setback");
                    }
                }
                // A login after a reconfiguration places a new player too.
                if matches!(packet.id, RESPAWN | LOGIN)
                    && let Backend::Java(java) = &mut self.backend
                {
                    java.setbacks.respawned = true;
                }
                // Boarding a boat turns the player to the boat's yaw.
                if packet.id == SET_PASSENGERS
                    && let Backend::Java(java) = &mut self.backend
                {
                    java.controls.yaw = java.client.movement.player.yaw;
                    java.setbacks.on_seat(java.client.movement.riding.is_some(), java.ticks);
                }
                let was_alive = self.state.player.alive;
                self.on_java_inventory(packet.id);
                self.on_java_health(&packet);
                self.on_java_dialog(packet.id);
                let message = match &mut self.backend {
                    Backend::Java(java) => java.client.message.take().map(crate::events::ChatMessage::from_java),
                    Backend::Bedrock(_) => None,
                };
                self.note_java_bed_refusal(message.as_ref());
                self.events.on_java_packet(message, &mut self.state, &mut self.out);
                // Java has no "searching for spawn": dying is the only trigger.
                self.schedule_respawn(was_alive);
                Step::Idle
            }
            Err(e) => {
                let reason = disconnect_reason(e);
                self.closed = Some(reason.clone());
                Step::Disconnected(reason)
            }
        }
    }

    fn on_java_health(&mut self, packet: &Packet) {
        use acacia_java_proto::Wire;
        if packet.id != acacia_java_proto::ids::play::clientbound::SET_HEALTH {
            return;
        }
        match acacia_java_proto::packets::SetHealth::read(&mut &packet.body[..]) {
            Ok(h) => self.state.player.set_vitals(h.health, h.food.0 as f32, h.saturation),
            Err(e) => tracing::debug!(error = %e, "bad SetHealth"),
        }
    }

    /// Mirrors the open dialog into `state.forms`; a new one is also a [`BotEvent::Form`].
    fn on_java_dialog(&mut self, id: i32) {
        use acacia_java_proto::ids::play::clientbound as from;
        let Backend::Java(java) = &self.backend else { return };
        if matches!(id, from::SHOW_DIALOG | from::CLEAR_DIALOG)
            && let Some(form) = self.state.forms.set_java(java.client.dialog.as_ref())
        {
            self.out.push(crate::BotEvent::Form(form));
        }
    }

    /// Mirrors the Java inventory after a packet that changed it; a server-set held slot is taken as the bot's.
    fn on_java_inventory(&mut self, id: i32) {
        use acacia_java_proto::ids::play::clientbound as from;
        let Backend::Java(java) = &self.backend else { return };
        let inventory = &java.client.inventory;
        match id {
            from::CONTAINER_SET_CONTENT | from::CONTAINER_SET_SLOT | from::SET_PLAYER_INVENTORY | from::SET_CURSOR_ITEM => {
                crate::state::java_inventory::mirror(&mut self.state.inventory, inventory);
                if matches!(id, from::CONTAINER_SET_CONTENT | from::CONTAINER_SET_SLOT) {
                    crate::state::java_inventory::mirror_window(&mut self.state.containers, inventory);
                }
            }
            from::CONTAINER_SET_DATA => crate::state::java_inventory::mirror_window(&mut self.state.containers, inventory),
            from::OPEN_SCREEN | from::CONTAINER_CLOSE => {
                crate::state::java_inventory::mirror_window(&mut self.state.containers, inventory);
                crate::state::java_trade::mirror_trade(&mut self.state.stations, inventory);
            }
            from::MERCHANT_OFFERS => crate::state::java_trade::mirror_trade(&mut self.state.stations, inventory),
            from::SET_HELD_SLOT => self.state.inventory.select(inventory.selected),
            _ => {}
        }
    }

    pub(super) fn java_tick(&mut self) {
        self.governor.tick(None);
        self.tick_respawn();
        self.tick_survival();
        // `Minecraft.tick` handles the held attack key before the player ticks.
        self.tick_mining();
        let dismounting = self.java_dismounting();
        let Backend::Java(java) = &mut self.backend else { return };
        java.ticks += 1;
        java.setbacks.respawned &= !java.client.loaded();
        let player = &mut java.client.movement.player;
        // Turned by the shorter way and never wrapped, as a mouse turns it: Grim's AimModulo360 flags a 360° jump.
        player.yaw += wrap_degrees(java.controls.yaw - player.yaw);
        player.pitch = java.controls.pitch.clamp(-90.0, 90.0);
        super::java_gear::apply_worn(player, &java.client.inventory);
        let mut keys = java_keys(&java.controls);
        keys.shift |= dismounting;
        if let Err(e) = java.client.step(keys) {
            tracing::debug!(error = %e, "java movement not sent");
        }
        self.state.fishing.mirror_java(java.client.world.own_bobber().map(|(id, e)| (id, e.position, e.biting)));
        let in_bed = java.client.bed().is_some();
        (self.state.player.sleep_flags[0], java.setbacks.in_bed) = (in_bed, in_bed);
        let p = &java.client.movement.player;
        tracing::trace!(target: "java_tick", name = %java.client.name, tick = java.ticks, pos = ?p.pos, delta = ?p.delta,
            ground = p.on_ground, hcol = p.horizontal_collision, sprinting = p.sprinting, speed = p.speed_value(), keys = ?keys);
        // A boat turns its rider with it.
        if java.client.movement.riding.is_some() {
            java.controls.yaw = java.client.movement.player.yaw;
        }
        if self.state.keeps_entities() {
            self.state.entities.advance();
            self.state.entities.mirror_java(&java.client.world.entities, i64::from(java.client.world.player_id));
        }
        self.mirror_java_riding();
    }
}

pub(crate) fn disconnect_reason(e: Error) -> DisconnectReason {
    match e {
        // The link's tasks ended without an error of their own: Client::close stopped them.
        Error::Closed => DisconnectReason::LocalClose,
        Error::Io(e) => DisconnectReason::Io(e.to_string()),
        Error::Disconnected(message) => DisconnectReason::Kicked { reason: String::new(), message },
        e => DisconnectReason::Protocol(e.to_string()),
    }
}
