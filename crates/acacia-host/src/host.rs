use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use acacia_api::{ApiError, BotId, Command, ErrorCode, Method};
use acacia_bot::snapshot::SnapshotReader;
use acacia_bot::swarm::{BotId as ShardJobId, Job, JoinQueue, OnPanic, Shard};
use acacia_bot::world::SharedWorlds;
use parking_lot::Mutex;

use crate::bot::{self, Spawn};
use crate::outbound::{self, Flusher, Outbound, ShardBuf};
use crate::registry::Registry;
use crate::sign_in::{MicrosoftSignIn, SignIn, SignIns};

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Shard threads (default: the CPU count, at most 4).
    pub shards: usize,
    /// Batches the outbound channel holds before shards hold records back (default 256).
    pub batches: usize,
    /// A shard sends events at most one batch per this long (default 10 ms), so bots that tick together share one.
    /// A call's `done` goes out at once, behind the events queued before it.
    pub flush_every: Duration,
    /// Records a shard holds back while the channel is full before it drops lossy events (default 65536).
    pub max_held: usize,
    /// Calls one bot may have open (queued, running or unanswered) before more are refused `BUSY` (default 64).
    pub max_open_calls: usize,
    /// Events a bot queues for its task before dropping lossy ones (default 4096).
    pub event_capacity: usize,
    /// Bots publish their state every tick for [`Host::hot`] and [`Host::block_at`] (default true).
    pub snapshots: bool,
    /// Largest decompressed batch a bot accepts before it drops the connection (default 128 MiB). A newcomer on
    /// a crowded server gets every player's skin in one batch: about 200 players overflowed the client's 32 MiB.
    pub max_batch_bytes: usize,
    /// Joins to one address start this far apart, across every bot of the host (default 500 ms): BDS refuses a
    /// join from an IP that joined moments ago (RakNet `RecentlyConnected`; 400 ms was enough for 10 bots).
    pub join_spacing: Duration,
    /// Each gap varies by up to this much either way, capped at `join_spacing` (default 250 ms).
    pub join_jitter: Duration,
    /// Where Microsoft sign-in goes (default `None`: [`MicrosoftSignIn`], acacia-auth's device-code flow).
    pub sign_in: Option<Arc<dyn SignIn>>,
    /// The token cache for calls that name no `profilesFolder` (default `None`: [`crate::default_profiles_folder`]).
    pub profiles_folder: Option<PathBuf>,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            shards: std::thread::available_parallelism().map_or(1, |n| n.get().min(4)),
            batches: 256,
            flush_every: Duration::from_millis(10),
            max_held: 65536,
            max_open_calls: 64,
            event_capacity: 4096,
            snapshots: true,
            max_batch_bytes: 128 << 20,
            join_spacing: JoinQueue::DELAY,
            join_jitter: JoinQueue::JITTER,
            sign_in: None,
            profiles_folder: None,
        }
    }
}

/// Runs bots for a transport (see the crate docs). `Send + Sync`: call it from any thread.
pub struct Host {
    registry: Arc<Mutex<Registry>>,
    shards: Mutex<Vec<Shard>>,
    bufs: Vec<Arc<ShardBuf>>,
    shared_worlds: SharedWorlds,
    joins: Arc<JoinQueue>,
    sign_ins: Arc<SignIns>,
    event_capacity: usize,
    snapshots: bool,
    max_batch_bytes: usize,
}

impl Host {
    /// Starts the shard threads; every record comes out of the returned [`Outbound`].
    pub fn start(config: HostConfig) -> io::Result<(Host, Outbound)> {
        let shards = config.shards.max(1);
        let (tx, outbound) = outbound::channel(config.batches.max(1));
        let registry = Arc::new(Mutex::new(Registry::new(shards, config.max_open_calls)));
        let on_panic: OnPanic = {
            let registry = registry.clone();
            Arc::new(move |id, message| registry.lock().panicked(&acacia_api::BotId(id.0.clone()), &message))
        };
        let (mut threads, mut bufs) = (Vec::new(), Vec::new());
        for index in 0..shards {
            let shard = Shard::start(index, on_panic.clone())?;
            let flusher = Flusher { buf: ShardBuf::new(), tx: tx.clone(), every: config.flush_every, max_held: config.max_held };
            bufs.push(flusher.buf.clone());
            shard.submit(ShardJobId(format!("flusher-{index}")), Box::new(move || Box::pin(flusher.run())));
            threads.push(shard);
        }
        let provider = config.sign_in.unwrap_or_else(|| Arc::new(MicrosoftSignIn::default()));
        let sign_ins = Arc::new(SignIns::new(provider, config.profiles_folder, bufs[0].clone()));
        let host = Host {
            registry,
            shards: Mutex::new(threads),
            bufs,
            shared_worlds: SharedWorlds::new(),
            joins: Arc::new(JoinQueue::new(config.join_spacing, config.join_jitter)),
            sign_ins,
            event_capacity: config.event_capacity,
            snapshots: config.snapshots,
            max_batch_bytes: config.max_batch_bytes,
        };
        Ok((host, outbound))
    }

    /// Takes one [`Command`] as JSON. `Err` means the call was refused and will get no `done`.
    pub fn call(&self, json: &str) -> Result<(), ApiError> {
        self.submit(Command::from_json(json).map_err(|(_, e)| e)?)
    }

    pub fn submit(&self, command: Command) -> Result<(), ApiError> {
        let Command { call, method } = command;
        let params = match method {
            Method::CreateBot(params) => params,
            Method::SignIn(params) => return self.sign_ins.sign_in(call, params),
            method => {
                let bot = method.bot().cloned().ok_or_else(|| ApiError::new(ErrorCode::BadParams, "no bot named"))?;
                return self.registry.lock().send(&bot, call, method);
            }
        };
        let (shard, out, rx) = self.registry.lock().add(call, &self.bufs)?;
        let id = ShardJobId(out.id.0.clone());
        let spawn = Spawn {
            out,
            rx,
            registry: self.registry.clone(),
            call,
            params,
            shared_worlds: self.shared_worlds.clone(),
            joins: self.joins.clone(),
            sign_ins: self.sign_ins.clone(),
            event_capacity: self.event_capacity,
            snapshots: self.snapshots,
            max_batch_bytes: self.max_batch_bytes,
        };
        let job: Job = Box::new(move || Box::pin(bot::run(spawn)));
        let submitted = self.shards.lock().get(shard).is_some_and(|s| s.submit(id.clone(), job));
        if !submitted {
            self.registry.lock().remove(&acacia_api::BotId(id.0));
            return Err(ApiError::new(ErrorCode::Closed, "the host has shut down"));
        }
        Ok(())
    }

    /// The bot's published state (see crate::reads); `None` for an unknown bot, one not spawned yet, or with
    /// [`HostConfig::snapshots`] off.
    pub(crate) fn reader(&self, bot: &BotId) -> Option<SnapshotReader> {
        self.registry.lock().reader(bot)
    }

    /// Closes every bot (as `bot.close`, waiting at most 10 s for them), sends what is buffered and joins the shard
    /// threads. Later calls are refused `CLOSED`; the [`Outbound`] ends once drained.
    pub async fn shutdown(&self) {
        let emptied = self.registry.lock().close_all();
        if let Some(emptied) = emptied {
            let _ = tokio::time::timeout(Duration::from_secs(10), emptied.notified()).await;
        }
        self.sign_ins.close();
        for buf in &self.bufs {
            buf.stop();
        }
        let shards = std::mem::take(&mut *self.shards.lock());
        for shard in shards {
            shard.stop().await;
        }
    }

    /// [`Host::shutdown`] from a thread outside any tokio runtime (it runs one of its own).
    pub fn shutdown_blocking(&self) {
        std::thread::scope(|s| {
            s.spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime for shutdown");
                runtime.block_on(self.shutdown());
            });
        });
    }
}
