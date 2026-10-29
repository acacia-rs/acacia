use std::sync::Arc;
use std::time::Duration;

use acacia_client::proto::types::InputData;
use acacia_client::proto::RawPacket;
use acacia_client::{Client, ClientBuilder, ConnectError, DisconnectReason, Event, MemoryBlobStore, PacketFilter};
use crate::cadence::Ticker;
use crate::events::{BotEvent, EventOut, EventSource, Events};
use crate::governor::Governor;
use crate::human::Human;
use crate::interact::closing::PendingClose;
use crate::lag::LagLine;
use crate::items::RequestIds;
use crate::movement::{Idle, Movement};
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

mod access;
mod dimension;
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
    pub(crate) events: EventSource,
    pub(crate) human: Human,
    pub(crate) survival: Survival,
    /// Events not yet returned by [`Bot::next`], or the sink they go to (see [`crate::events`]).
    pub(crate) out: EventOut,
    /// Flags for the next `PlayerAuthInput` (item use, gliding, dismount sneak).
    pub(crate) queued_flags: Vec<InputData>,
    pub(crate) ride: Ride,
    pub(crate) bed: Bed,
    pub(crate) request_ids: RequestIds,
    pub(crate) reflexes: Reflexes,
    pub(crate) mining: Option<crate::interact::Mining>,
    /// A dimension change in progress.
    travel: Option<dimension::Travel>,
    /// Physics bots: the simulation has started once, and the spawn packets went out with it.
    spawned: bool,
    strict: bool,
    /// A client container close still settling (interact/closing.rs).
    pub(crate) closing: Option<PendingClose>,
    closed: Option<DisconnectReason>,
    pub(crate) governor: Governor,
    pub(crate) act_timing: crate::act_timing::ActTiming,
    pub(crate) in_flight: crate::interact::pipeline::Pipeline<crate::interact::pipeline::StackUndo>,
    lag: Option<LagLine>,
    pub(crate) cancel: crate::cancel::CancelState,
    pub(crate) inbox: crate::inbox::InboxState,
    pub(crate) snapshots: Option<crate::snapshot::Publisher>,
}

impl Bot {
    /// Connects and spawns. Tracked packets are added to `builder`'s subscription automatically.
    pub async fn connect(builder: ClientBuilder, mut config: BotConfig) -> Result<Bot, ConnectError> {
        config.trackers.block_entities = config.trackers.block_entities.resolve(config.physics);
        let mut tracked: PacketFilter = config.trackers.packet_ids().into_iter().collect();
        tracked = PredictionSync::PACKETS.iter().chain(crate::sleep::PACKETS).chain(dimension::PACKETS).fold(tracked, |f, &id| f.with(id));
        tracked = WorldTracker::PACKETS.iter().fold(tracked, |f, &id| f.with(id));
        let mode = if config.physics { Movement::PACKETS } else { SubChunkRequester::PACKETS };
        tracked = mode.iter().fold(tracked, |f, &id| f.with(id));
        if config.physics && config.record.is_some() {
            tracked = trace::ANALYSIS_PACKETS.iter().fold(tracked, |f, &id| f.with(id));
        }
        let server = builder.server().to_owned();
        // Physics reads terrain from blobs; a trace must hold every blob it uses, so it gets a fresh store.
        let mut builder = builder.keep_blob_payloads(config.physics).mouse_input(config.mouse_input);
        if config.physics && config.record.is_some() {
            builder = builder.blob_store(Arc::new(MemoryBlobStore::with_payloads()));
        }
        // The bot sends SetLocalPlayerAsInitialized itself, when it leaves the loading screen (spawn.rs).
        let events = EventSource::new(config.events, config.chat_patterns).with_base(tracked.union(&config.subscribe));
        let client = builder.filter(events.filter()).initialize_on_spawn(false).strict(config.strict).connect().await?;
        let state = GameState::new(config.trackers, client.runtime_entity_id());
        let recorder = config.record.as_deref().filter(|_| config.physics).and_then(|path| {
            Recorder::create(path).inspect_err(|e| tracing::warn!(error = %e, "cannot record trace")).ok()
        });
        let mut world = WorldTracker::new(server, config.shared_worlds);
        if let Some(store) = client.blob_store() {
            world.set_blob_store(store.clone());
        }
        let (world, movement, idle) = if config.physics {
            let mut movement = Movement::new();
            movement.mouse_input = config.mouse_input;
            movement.rotation_noise = Some(crate::movement::RotationNoise::new(crate::cadence::seed()));
            (world, Some(movement), None)
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
            out: EventOut::new(config.event_sink),
            queued_flags: Vec::new(),
            ride: Ride::default(),
            bed: Bed::default(),
            request_ids: RequestIds::default(),
            reflexes: Reflexes::default(),
            mining: None,
            travel: None,
            spawned: false,
            strict: config.strict,
            closing: None,
            closed: None,
            governor: Governor::default(),
            act_timing: Default::default(),
            in_flight: Default::default(),
            lag: config.lag.map(LagLine::new),
            cancel: Default::default(),
            inbox: Default::default(),
            snapshots: config.snapshots.then(Default::default),
        })
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
    /// the connection ends. Must be polled continuously for state, input and movement to run. With an
    /// event sink it returns only `None`, once the connection is gone (see [`crate::events`]).
    pub async fn next(&mut self) -> Option<BotEvent> {
        self.idle_again();
        loop {
            if let Some(event) = self.out.pop() {
                return Some(event);
            }
            self.survival.idle = true;
            match self.step().await? {
                Step::Packet(packet) => self.keep_for_caller(packet),
                Step::Tick if !self.out.is_sink() && self.events.wants(Events::TICKS) => return Some(BotEvent::Tick),
                Step::Tick | Step::Idle => {}
                Step::Disconnected(reason) => return self.out.hand_over(BotEvent::Disconnected(reason)),
            }
        }
    }

    /// Handles one incoming packet or movement tick, updating state. `None` once the connection is gone.
    pub(crate) async fn step(&mut self) -> Option<Step> {
        self.drain_inbox();
        if let Some(world) = &mut self.world {
            for request in world.outgoing.drain(..) {
                self.client.send(&request);
            }
            let rejected = world.rejected.drain(..).filter(|_| self.strict);
            self.out.extend(rejected.map(BotEvent::Violation));
        }
        if let Some(packet) = self.lag.as_mut().and_then(|l| l.pop_due(tokio::time::Instant::now())) {
            self.apply(&packet);
            return Some(Step::Packet(packet));
        }
        // A queued event with no tick due skips the select!, whose tick timer was registered and cleared per packet.
        if self.lag.is_none()
            && !self.ticker.due()
            && let Some(event) = self.client.try_recv()
        {
            return self.on_event(event);
        }
        let lagged = self.lag.as_ref().and_then(LagLine::next_at);
        tokio::select! {
            event = self.client.recv() => self.on_event(event?),
            // In an async block: select! builds a disabled branch's future too, and a bare sleep looks up the timer.
            _ = async { tokio::time::sleep_until(lagged.unwrap_or_else(tokio::time::Instant::now)).await }, if lagged.is_some() => Some(Step::Idle),
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

    fn on_event(&mut self, event: Event) -> Option<Step> {
        match event {
            Event::Packet(packet) if self.lag.is_some() => {
                self.lag.as_mut()?.push(tokio::time::Instant::now(), packet);
                Some(Step::Idle)
            }
            Event::Packet(packet) => {
                self.apply(&packet);
                Some(Step::Packet(packet))
            }
            Event::Violation(violation) => {
                self.out.push(BotEvent::Violation(violation));
                Some(Step::Idle)
            }
            Event::Disconnected(reason) => {
                self.closed = Some(reason.clone());
                Some(Step::Disconnected(reason))
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
        self.on_container_close(packet);
        self.sync.apply(packet, self.state.player.runtime_entity_id);
        if let Some(subchunks) = &mut self.subchunks {
            subchunks.apply(packet);
        }
        self.on_travel_packet(packet);
        self.auto_respawn(packet, was_alive);
        self.reflex_packet(packet);
        self.on_fetch_packet(packet);
        self.events.on_packet(packet, &mut self.state, &mut self.out);
    }
}
