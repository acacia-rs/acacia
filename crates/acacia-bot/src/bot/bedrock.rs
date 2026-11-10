//! The Bedrock Edition side of a [`Bot`]: the connection and what reads and writes its packets.

use std::sync::Arc;

use acacia_client::proto::types::InputData;
use acacia_client::{Client, ClientBuilder, ConnectError, MemoryBlobStore, PacketFilter, PacketTraceSummary};

use super::dimension::{self, Travel};
use crate::events::EventSource;
use crate::interact::closing::PendingClose;
use crate::interact::pipeline::{Pipeline, StackUndo};
use crate::items::RequestIds;
use crate::lag::LagLine;
use crate::movement::{Idle, Movement};
use crate::prediction_sync::PredictionSync;
use crate::spawn::SpawnSequence;
use crate::state::GameState;
use crate::subchunks::SubChunkRequester;
use crate::trace::{self, Recorder};
use crate::wait::ActionError;
use crate::world::WorldTracker;
use crate::{Bot, BotConfig};

/// A Bedrock connection plus the trackers and pipelines bound to its wire format.
pub struct Bedrock {
    pub(crate) client: Client,
    pub(crate) subscribe: PacketFilter,
    pub(crate) world: Option<WorldTracker>,
    pub(crate) movement: Option<Movement>,
    /// Bots without physics: standing-still input and the loading-screen sequence.
    pub(crate) idle: Option<(Idle, SpawnSequence)>,
    /// Bots without physics: vanilla's sub-chunk requests (physics bots request through `world`).
    pub(crate) subchunks: Option<SubChunkRequester>,
    pub(crate) sync: PredictionSync,
    pub(crate) recorder: Option<Recorder>,
    /// Flags for the next `PlayerAuthInput` (item use, gliding, dismount sneak).
    pub(crate) queued_flags: Vec<InputData>,
    pub(crate) request_ids: RequestIds,
    pub(crate) in_flight: Pipeline<StackUndo>,
    /// A dimension change in progress.
    pub(crate) travel: Option<Travel>,
    /// Physics bots: the simulation has started once, and the spawn packets went out with it.
    pub(crate) spawned: bool,
    pub(crate) strict: bool,
    /// A client container close still settling (interact/closing.rs).
    pub(crate) closing: Option<PendingClose>,
    pub(crate) lag: Option<LagLine>,
}

impl Bedrock {
    /// Connects and spawns. Tracked packets are added to `builder`'s subscription automatically.
    pub(super) async fn connect(builder: ClientBuilder, mut config: BotConfig) -> Result<Bot, ConnectError> {
        config.trackers.block_entities = config.trackers.block_entities.resolve(config.physics);
        let events = EventSource::new(config.events, config.chat_patterns);
        let mut tracked: PacketFilter = config.trackers.packet_ids().into_iter().chain(events.packet_ids()).collect();
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
            let mut movement = Movement::new();
            movement.mouse_input = config.mouse_input;
            movement.rotation_noise = Some(crate::movement::RotationNoise::new(crate::cadence::seed()));
            (world, Some(movement), None)
        } else {
            (world.nearby(), None, Some((Idle::default(), SpawnSequence::default())))
        };
        let bedrock = Bedrock {
            client,
            subscribe: config.subscribe,
            world: Some(world),
            movement,
            subchunks: idle.is_some().then(SubChunkRequester::default),
            idle,
            sync: PredictionSync::default(),
            recorder,
            queued_flags: Vec::new(),
            request_ids: RequestIds::default(),
            in_flight: Default::default(),
            travel: None,
            spawned: false,
            strict: config.strict,
            closing: None,
            lag: config.lag.map(LagLine::new),
        };
        Ok(Bot::assemble(Backend::Bedrock(Box::new(bedrock)), state, events, config.auto_respawn, config.auto_eat))
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn world(&self) -> Option<&WorldTracker> {
        self.world.as_ref()
    }

    /// The Bedrock physics (physics bots only).
    pub fn movement(&self) -> Option<&Movement> {
        self.movement.as_ref()
    }

    /// Recent inbound packets (filtered ones too) and the server's silence, for stall diagnostics.
    pub fn packet_trace(&self) -> PacketTraceSummary {
        self.client.packet_trace()
    }

    /// Labels the recorded trace from here on (no-op when not recording).
    pub fn trace_mark(&mut self, label: &str) {
        if let Some(r) = &mut self.recorder {
            r.write(&trace::Event::Mark(label.to_owned()));
        }
    }
}

/// The edition a [`Bot`] is connected with.
pub(crate) enum Backend {
    Bedrock(Box<Bedrock>),
    #[cfg(feature = "java")]
    Java(Box<super::java::Java>),
}

impl Backend {
    pub(crate) fn bedrock(&self) -> Option<&Bedrock> {
        match self {
            Backend::Bedrock(b) => Some(b),
            #[cfg(feature = "java")]
            Backend::Java(_) => None,
        }
    }

    pub(crate) fn bedrock_mut(&mut self) -> Option<&mut Bedrock> {
        match self {
            Backend::Bedrock(b) => Some(b),
            #[cfg(feature = "java")]
            Backend::Java(_) => None,
        }
    }

    /// The Bedrock side, or [`ActionError::Unsupported`] naming `what` needed it.
    pub(crate) fn require(&self, what: &'static str) -> Result<&Bedrock, ActionError> {
        self.bedrock().ok_or(ActionError::Unsupported(what))
    }

    /// Like [`Backend::require`], mutably.
    pub(crate) fn require_mut(&mut self, what: &'static str) -> Result<&mut Bedrock, ActionError> {
        self.bedrock_mut().ok_or(ActionError::Unsupported(what))
    }

    /// The Bedrock physics; `None` for bots without physics and on Java.
    pub(crate) fn movement(&self) -> Option<&Movement> {
        self.bedrock()?.movement.as_ref()
    }

    pub(crate) fn movement_mut(&mut self) -> Option<&mut Movement> {
        self.bedrock_mut()?.movement.as_mut()
    }

    /// The Bedrock world; `None` on Java.
    pub(crate) fn world(&self) -> Option<&WorldTracker> {
        self.bedrock()?.world.as_ref()
    }

    /// World and physics together, for a physics bot on Bedrock.
    pub(crate) fn physics(&self) -> Option<(&WorldTracker, &Movement)> {
        let b = self.bedrock()?;
        Some((b.world.as_ref()?, b.movement.as_ref()?))
    }

    pub(crate) fn physics_mut(&mut self) -> Option<(&WorldTracker, &mut Movement)> {
        let b = self.bedrock_mut()?;
        Some((b.world.as_ref()?, b.movement.as_mut()?))
    }

    pub(crate) fn idle(&self) -> Option<&(Idle, SpawnSequence)> {
        self.bedrock()?.idle.as_ref()
    }

    pub(crate) fn idle_mut(&mut self) -> Option<&mut (Idle, SpawnSequence)> {
        self.bedrock_mut()?.idle.as_mut()
    }
}
