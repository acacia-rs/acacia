use std::collections::VecDeque;
use std::fmt::Debug;
use std::time::Duration;

use acacia_client::proto::{Packet, RawPacket};
use acacia_client::{ClientBuilder, ConnectError, DisconnectReason};
use crate::cadence::Ticker;
use crate::events::{BotEvent, EventSource, Events};
use crate::governor::Governor;
use crate::human::Human;
use crate::movement::Controls;
use crate::reflex::Reflexes;
use crate::riding::Ride;
use crate::sleep::Bed;
use crate::state::GameState;
use crate::survival::{AutoEat, Survival};
use crate::BotConfig;

mod bedrock;
mod bedrock_step;
mod dimension;
#[cfg(feature = "java")]
mod java;
mod tick;

pub(crate) use bedrock::Backend;
pub use bedrock::Bedrock;
#[cfg(feature = "java")]
pub use java::Java;

/// One unit of work done by [`Bot::step`].
pub(crate) enum Step {
    Packet(RawPacket),
    Tick,
    /// Nothing for an action to react to (an event was queued).
    Idle,
    Disconnected(DisconnectReason),
}

/// A connected client plus the game state tracked from its packets.
pub struct Bot {
    pub(crate) backend: Backend,
    pub(crate) state: GameState,
    ticker: Ticker,
    pub(crate) auto_respawn: bool,
    /// Ticks until the scheduled respawn (respawn.rs).
    pub(crate) respawn_in: Option<u32>,
    events: EventSource,
    pub(crate) human: Human,
    pub(crate) survival: Survival,
    /// Events not yet returned by [`Bot::next`] (queued while stepping or while an action waited).
    pub(crate) pending: VecDeque<BotEvent>,
    pub(crate) ride: Ride,
    pub(crate) bed: Bed,
    pub(crate) reflexes: Reflexes,
    pub(crate) mining: Option<crate::interact::Mining>,
    closed: Option<DisconnectReason>,
    pub(crate) governor: Governor,
    pub(crate) act_timing: crate::act_timing::ActTiming,
    pub(crate) cancel: crate::cancel::CancelState,
}

impl Bot {
    /// Connects to a Bedrock server and spawns. Tracked packets are added to `builder`'s subscription automatically.
    pub async fn connect(builder: ClientBuilder, config: BotConfig) -> Result<Bot, ConnectError> {
        Bedrock::connect(builder, config).await
    }

    fn assemble(backend: Backend, state: GameState, events: EventSource, auto_respawn: bool, auto_eat: Option<AutoEat>) -> Bot {
        Bot {
            backend,
            state,
            ticker: Ticker::new(),
            auto_respawn,
            respawn_in: None,
            events,
            human: Human::default(),
            survival: Survival::new(auto_eat),
            pending: VecDeque::new(),
            ride: Ride::default(),
            bed: Bed::default(),
            reflexes: Reflexes::default(),
            mining: None,
            closed: None,
            governor: Governor::default(),
            act_timing: Default::default(),
            cancel: Default::default(),
        }
    }

    /// Why the connection ended, once it has.
    pub fn disconnect_reason(&self) -> Option<&DisconnectReason> {
        self.closed.as_ref()
    }

    /// The Bedrock side (connection, world, physics), for Bedrock bots.
    pub fn bedrock(&self) -> Option<&Bedrock> {
        self.backend.bedrock()
    }

    pub fn bedrock_mut(&mut self) -> Option<&mut Bedrock> {
        self.backend.bedrock_mut()
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// Movement intent (physics bots and Java bots); applied from the next tick on.
    pub fn controls(&mut self) -> Option<&mut Controls> {
        match &mut self.backend {
            Backend::Bedrock(b) => b.movement.as_mut().map(|m| &mut m.controls),
            #[cfg(feature = "java")]
            Backend::Java(java) => Some(&mut java.controls),
        }
    }

    /// The player's name on the server.
    pub fn name(&self) -> &str {
        match &self.backend {
            Backend::Bedrock(b) => b.client.display_name(),
            #[cfg(feature = "java")]
            Backend::Java(java) => &java.client.name,
        }
    }

    /// Runs a command (the leading `/` is optional); false if it could not be sent.
    pub fn command(&self, cmd: &str) -> bool {
        match &self.backend {
            Backend::Bedrock(b) => b.client.command(cmd),
            #[cfg(feature = "java")]
            Backend::Java(_) => java::not_yet("commands"),
        }
    }

    /// Says `text` in chat; false if it could not be sent.
    pub fn chat(&self, text: &str) -> bool {
        match &self.backend {
            Backend::Bedrock(b) => b.client.chat(text),
            #[cfg(feature = "java")]
            Backend::Java(_) => java::not_yet("chat"),
        }
    }

    /// Starts closing the connection; [`Bot::next`] then reports the disconnect.
    pub fn close(&self) {
        match &self.backend {
            Backend::Bedrock(b) => b.client.close(),
            #[cfg(feature = "java")]
            Backend::Java(java) => java.client.close(),
        }
    }

    /// Queues a Bedrock packet; false once the connection is closed, and always on Java (nothing is sent).
    pub(crate) fn send_bedrock<T: Packet + Debug>(&self, packet: &T) -> bool {
        self.backend.bedrock().is_some_and(|b| b.client.send(packet))
    }

    /// Closes the connection and waits (up to 2 s) for the disconnect to be sent, so the server
    /// frees the session at once instead of timing it out ("already logged in" on a quick relog).
    pub async fn disconnect(&mut self) {
        self.close();
        let closed = async {
            while let Some(event) = self.next().await {
                if matches!(event, BotEvent::Disconnected(_)) {
                    break;
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(2), closed).await;
    }

    /// Processes incoming packets and ticks until an event (typed or a subscribed packet) is ready or
    /// the connection ends. Must be polled continuously for state, input and movement to run.
    pub async fn next(&mut self) -> Option<BotEvent> {
        self.idle_again();
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            self.survival.idle = true;
            match self.step().await? {
                Step::Packet(packet) => self.keep_for_caller(packet),
                Step::Tick if self.events.wants(Events::TICKS) => return Some(BotEvent::Tick),
                Step::Tick | Step::Idle => {}
                Step::Disconnected(reason) => return Some(BotEvent::Disconnected(reason)),
            }
        }
    }

    /// Handles one incoming packet or movement tick, updating state. `None` once the connection is gone.
    pub(crate) async fn step(&mut self) -> Option<Step> {
        match self.backend {
            Backend::Bedrock(_) => self.bedrock_step().await,
            #[cfg(feature = "java")]
            Backend::Java(_) => self.java_step().await,
        }
    }
}
