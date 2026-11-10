//! The Java Edition side of a [`Bot`] (docs/java-edition.md, build step 4): it joins, ticks and walks by
//! [`Controls`]; actions that need the Bedrock wire return [`crate::ActionError::Unsupported`].

use acacia_client::DisconnectReason;
use acacia_java_client::{Client, Error, Packet};
use acacia_java_physics::Keys;
use acacia_java_proto::ids::play::clientbound::PLAYER_POSITION;

use super::bedrock::Backend;
use super::Step;
use crate::events::EventSource;
use crate::movement::{java_keys, Controls, JavaSim, Live};
use crate::state::GameState;
use crate::{Bot, BotConfig};

/// A Java connection; its client applies the world and movement packets itself.
pub struct Java {
    pub(crate) client: Client,
    pub(crate) controls: Controls,
    /// Sequence of the predicting action awaiting the server's answer, 0 for none.
    pub(crate) awaiting: i32,
    ticks: u64,
    teleports: u32,
    last_setback: Option<u64>,
}

impl Java {
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) fn started(&self) -> bool {
        self.client.loaded()
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
        self.teleports
    }

    fn setback_age(&self) -> Option<u64> {
        self.last_setback.map(|t| self.ticks.saturating_sub(t))
    }

    fn controls(&mut self) -> &mut Controls {
        &mut self.controls
    }
}

impl Bot {
    /// Joins an offline-mode Java server as `name`. Only `config`'s edition-neutral settings apply.
    pub async fn connect_java_offline(host: &str, port: u16, name: &str, config: BotConfig) -> Result<Bot, Error> {
        let client = Client::join_offline(host, port, name).await?;
        let events = EventSource::new(config.events, config.chat_patterns);
        let state = GameState::new(config.trackers, 0);
        let java = Java { client, controls: Controls::default(), awaiting: 0, ticks: 0, teleports: 0, last_setback: None };
        Ok(Bot::assemble(Backend::Java(Box::new(java)), state, events, config.auto_respawn, config.auto_eat))
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
        let Backend::Java(java) = &mut self.backend else { return None };
        if !self.ticker.due()
            && let Some(result) = java.client.try_next()
        {
            return Some(self.on_java_packet(result));
        }
        tokio::select! {
            result = java.client.next() => Some(self.on_java_packet(result)),
            ticks = self.ticker.wait() => {
                (0..ticks).for_each(|_| self.java_tick());
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
                    if java.teleports > 0 {
                        java.last_setback = Some(java.ticks);
                    }
                    java.teleports += 1;
                }
                Step::Idle
            }
            Err(e) => {
                let reason = disconnect_reason(e);
                self.closed = Some(reason.clone());
                Step::Disconnected(reason)
            }
        }
    }

    fn java_tick(&mut self) {
        self.governor.tick(None);
        let Backend::Java(java) = &mut self.backend else { return };
        java.ticks += 1;
        let player = &mut java.client.movement.player;
        (player.yaw, player.pitch) = (java.controls.yaw, java.controls.pitch.clamp(-90.0, 90.0));
        if let Err(e) = java.client.step(java_keys(&java.controls)) {
            tracing::debug!(error = %e, "java movement not sent");
        }
    }
}

fn disconnect_reason(e: Error) -> DisconnectReason {
    match e {
        // The link's tasks ended without an error of their own: Client::close stopped them.
        Error::Closed => DisconnectReason::LocalClose,
        Error::Io(e) => DisconnectReason::Io(e.to_string()),
        Error::Disconnected(message) => DisconnectReason::Kicked { reason: String::new(), message },
        e => DisconnectReason::Protocol(e.to_string()),
    }
}
