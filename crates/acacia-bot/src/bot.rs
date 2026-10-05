use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use acacia_client::proto::packets::ContainerClose;
use acacia_client::proto::types::InputData;
use acacia_client::proto::RawPacket;
use acacia_client::{Client, ClientBuilder, ConnectError, DisconnectReason, Event, MemoryBlobStore, PacketFilter};
use crate::cadence::Ticker;
use crate::events::{BotEvent, EventSource};
use crate::human::Human;
use crate::items::RequestIds;
use crate::movement::{Controls, Idle, Movement};
use crate::prediction_sync::PredictionSync;
use crate::reflex::Reflexes;
use crate::riding::Ride;
use crate::sleep::Bed;
use crate::subchunks::SubChunkRequester;
use crate::spawn::SpawnSequence;
use crate::state::{GameState, PlayerState};
use crate::survival::Survival;
use crate::trace::{self, Recorder};
use crate::world::WorldTracker;
use crate::BotConfig;

mod tick;

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
    pub(crate) client: Client,
    pub(crate) state: GameState,
    pub(crate) subscribe: PacketFilter,
    pub(crate) world: Option<WorldTracker>,
    pub(crate) movement: Option<Movement>,
    /// Bots without physics: standing-still input and the loading-screen sequence.
    pub(crate) idle: Option<(Idle, SpawnSequence)>,
    /// Bots without physics: vanilla's sub-chunk requests (physics bots request through `world`).
    subchunks: Option<SubChunkRequester>,
    sync: PredictionSync,
    ticker: Ticker,
    pub(crate) auto_respawn: bool,
    /// Ticks until the scheduled respawn (respawn.rs).
    pub(crate) respawn_in: Option<u32>,
    recorder: Option<Recorder>,
    events: EventSource,
    pub(crate) human: Human,
    pub(crate) survival: Survival,
    /// Events not yet returned by [`Bot::next`] (queued while stepping or while an action waited).
    pub(crate) pending: VecDeque<BotEvent>,
    /// Flags for the next `PlayerAuthInput` (item use, gliding, dismount sneak).
    pub(crate) queued_flags: Vec<InputData>,
    pub(crate) ride: Ride,
    pub(crate) bed: Bed,
    pub(crate) request_ids: RequestIds,
    pub(crate) reflexes: Reflexes,
    strict: bool,
    closed: Option<DisconnectReason>,
}

impl Bot {
    /// Connects and spawns. Tracked packets are added to `builder`'s subscription automatically.
    pub async fn connect(builder: ClientBuilder, mut config: BotConfig) -> Result<Bot, ConnectError> {
        config.trackers.block_entities = config.trackers.block_entities.resolve(config.physics);
        let events = EventSource::new(config.events, config.chat_patterns);
        let mut tracked: PacketFilter = config.trackers.packet_ids().into_iter().chain(events.packet_ids()).collect();
        tracked = PredictionSync::PACKETS.iter().chain(crate::sleep::PACKETS).fold(tracked, |f, &id| f.with(id));
        tracked = WorldTracker::PACKETS.iter().fold(tracked, |f, &id| f.with(id));
        let mode = if config.physics { Movement::PACKETS } else { SubChunkRequester::PACKETS };
        tracked = mode.iter().fold(tracked, |f, &id| f.with(id));
        if config.physics && config.record.is_some() {
            tracked = trace::ANALYSIS_PACKETS.iter().fold(tracked, |f, &id| f.with(id));
        }
        let server = builder.server().to_owned();
        // Physics reads terrain from blobs; a trace must hold every blob it uses, so it gets a fresh store.
        let mut builder = builder.keep_blob_payloads(config.physics);
        if config.physics && config.record.is_some() {
            builder = builder.blob_store(Arc::new(MemoryBlobStore::with_payloads()));
        }
        // The bot sends SetLocalPlayerAsInitialized itself, when it leaves the loading screen (spawn.rs).
        let client = builder.filter(tracked.union(&config.subscribe)).initialize_on_spawn(false).strict(config.strict).connect().await?;
        let state = GameState::new(config.trackers, client.runtime_entity_id());
        let recorder = config.record.as_deref().filter(|_| config.physics).and_then(|path| {
            Recorder::create(path).inspect_err(|e| tracing::warn!(error = %e, "cannot record trace")).ok()
        });
        let mut world = WorldTracker::new(server, config.shared_worlds);
        if let Some(store) = client.blob_store() {
            world.set_blob_store(store.clone());
        }
        let (world, movement, idle) = if config.physics {
            (world, Some(Movement::new()), None)
        } else {
            (world.nearby(), None, Some((Idle::default(), SpawnSequence::default())))
        };
        Ok(Bot {
            client,
            state,
            subscribe: config.subscribe,
            world: Some(world),
            movement,
            subchunks: idle.is_some().then(SubChunkRequester::default),
            idle,
            sync: PredictionSync::default(),
            ticker: Ticker::new(),
            auto_respawn: config.auto_respawn,
            respawn_in: None,
            recorder,
            events,
            human: Human::default(),
            survival: Survival::new(config.auto_eat),
            pending: VecDeque::new(),
            queued_flags: Vec::new(),
            ride: Ride::default(),
            bed: Bed::default(),
            request_ids: RequestIds::default(),
            reflexes: Reflexes::default(),
            strict: config.strict,
            closed: None,
        })
    }

    /// Why the connection ended, once it has.
    pub fn disconnect_reason(&self) -> Option<&DisconnectReason> {
        self.closed.as_ref()
    }

    /// Labels the recorded trace from here on (no-op when not recording).
    pub fn trace_mark(&mut self, label: &str) {
        if let Some(r) = &mut self.recorder {
            r.write(&trace::Event::Mark(label.to_owned()));
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn world(&self) -> Option<&WorldTracker> {
        self.world.as_ref()
    }

    /// Movement intent (physics bots only); applied from the next tick on.
    pub fn controls(&mut self) -> Option<&mut Controls> {
        self.movement.as_mut().map(|m| &mut m.controls)
    }

    pub fn movement(&self) -> Option<&Movement> {
        self.movement.as_ref()
    }

    /// Closes the connection and waits (up to 2 s) for the disconnect to be sent, so the server
    /// frees the session at once instead of timing it out ("already logged in" on a quick relog).
    pub async fn disconnect(&mut self) {
        self.client.close();
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
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            self.survival.idle = true;
            match self.step().await? {
                Step::Packet(packet) => self.keep_for_caller(packet),
                Step::Tick | Step::Idle => {}
                Step::Disconnected(reason) => return Some(BotEvent::Disconnected(reason)),
            }
        }
    }

    /// Handles one incoming packet or movement tick, updating state. `None` once the connection is gone.
    pub(crate) async fn step(&mut self) -> Option<Step> {
        if let Some(world) = &mut self.world {
            for request in world.outgoing.drain(..) {
                self.client.send(&request);
            }
            let rejected = world.rejected.drain(..).filter(|_| self.strict);
            self.pending.extend(rejected.map(BotEvent::Violation));
        }
        tokio::select! {
            event = self.client.recv() => match event? {
                Event::Packet(packet) => {
                    self.apply(&packet);
                    Some(Step::Packet(packet))
                }
                Event::Violation(violation) => {
                    self.pending.push_back(BotEvent::Violation(violation));
                    Some(Step::Idle)
                }
                Event::Disconnected(reason) => {
                    self.closed = Some(reason.clone());
                    Some(Step::Disconnected(reason))
                }
            },
            ticks = self.ticker.wait() => {
                (0..ticks).for_each(|_| self.on_tick());
                Some(Step::Tick)
            }
        }
    }

    fn apply(&mut self, packet: &RawPacket) {
        if let Some(r) = &mut self.recorder
            && (PlayerState::PACKETS.contains(&packet.id)
                || WorldTracker::PACKETS.contains(&packet.id)
                || Movement::PACKETS.contains(&packet.id)
                || trace::ANALYSIS_PACKETS.contains(&packet.id) && trace::about(packet, self.state.me().runtime_entity_id))
        {
            r.write(&trace::Event::Packet(packet.clone()));
        }
        let was_alive = self.state.player.alive;
        let mut result = self.state.apply(packet);
        let me = self.state.me();
        if let Some(world) = &mut self.world {
            result = result.and(world.apply(packet));
        }
        if let Some(movement) = &mut self.movement {
            result = result.and(movement.apply(packet, &me));
        }
        if let Err(e) = result {
            tracing::debug!(id = packet.id, error = %e, "tracker failed to decode packet");
        }
        // Acknowledge server-initiated closes like vanilla: Geyser opens no further window until it gets this.
        if let Ok(close) = packet.decode::<ContainerClose>()
            && close.server
        {
            self.client.send(&ContainerClose { server: false, ..close });
        }
        self.sync.apply(packet, self.state.player.runtime_entity_id);
        if let Some(subchunks) = &mut self.subchunks {
            subchunks.apply(packet);
        }
        self.auto_respawn(packet, was_alive);
        self.reflex_packet(packet);
        self.on_fetch_packet(packet);
        self.events.on_packet(packet, &mut self.state, &mut self.pending);
    }
}
