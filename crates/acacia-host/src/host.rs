use std::io;
use std::sync::Arc;
use std::time::Duration;

use acacia_api::{ApiError, Block, BlockPos, BotId, Command, ErrorCode, HotSnapshot, Method};
use acacia_bot::swarm::{BotId as ShardJobId, Job, OnPanic, Shard};
use acacia_bot::world::SharedWorlds;
use parking_lot::Mutex;

use crate::bot::{self, Spawn};
use crate::convert;
use crate::outbound::{self, Flusher, Outbound, ShardBuf};
use crate::registry::Registry;

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Shard threads (default: the CPU count, at most 4).
    pub shards: usize,
    /// Batches the outbound channel holds before shards hold records back (default 256).
    pub batches: usize,
    /// A shard sends at most one batch per this long (default 10 ms), so bots that tick together share one.
    pub flush_every: Duration,
    /// Records a shard holds back while the channel is full before it drops lossy events (default 65536).
    pub max_held: usize,
    /// Calls one bot may have open (queued, running or unanswered) before more are refused `BUSY` (default 64).
    pub max_open_calls: usize,
    /// Events a bot queues for its task before dropping lossy ones (default 4096).
    pub event_capacity: usize,
    /// Bots publish their state every tick for [`Host::hot`] and [`Host::block_at`] (default true).
    pub snapshots: bool,
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
        }
    }
}

/// Runs bots for a transport (see the crate docs). `Send + Sync`: call it from any thread.
pub struct Host {
    registry: Arc<Mutex<Registry>>,
    shards: Mutex<Vec<Shard>>,
    bufs: Vec<Arc<ShardBuf>>,
    shared_worlds: SharedWorlds,
    event_capacity: usize,
    snapshots: bool,
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
        let host = Host {
            registry,
            shards: Mutex::new(threads),
            bufs,
            shared_worlds: SharedWorlds::new(),
            event_capacity: config.event_capacity,
            snapshots: config.snapshots,
        };
        Ok((host, outbound))
    }

    /// Takes one [`Command`] as JSON. `Err` means the call was refused and will get no `done`.
    pub fn call(&self, json: &str) -> Result<(), ApiError> {
        self.submit(Command::from_json(json).map_err(|(_, e)| e)?)
    }

    pub fn submit(&self, command: Command) -> Result<(), ApiError> {
        let Command { call, method } = command;
        let Method::CreateBot(params) = method else {
            let bot = method.bot().cloned().ok_or_else(|| ApiError::new(ErrorCode::BadParams, "no bot named"))?;
            return self.registry.lock().send(&bot, call, method);
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
            event_capacity: self.event_capacity,
            snapshots: self.snapshots,
        };
        let job: Job = Box::new(move || Box::pin(bot::run(spawn)));
        let submitted = self.shards.lock().get(shard).is_some_and(|s| s.submit(id.clone(), job));
        if !submitted {
            self.registry.lock().remove(&acacia_api::BotId(id.0));
            return Err(ApiError::new(ErrorCode::Closed, "the host has shut down"));
        }
        Ok(())
    }

    /// The bot's state as of its last tick, read without a call; `None` for an unknown bot, one not spawned yet,
    /// or with [`HostConfig::snapshots`] off.
    pub fn hot(&self, bot: &BotId) -> Option<HotSnapshot> {
        let reader = self.registry.lock().reader(bot)?;
        Some(convert::hot(&reader.hot()))
    }

    /// The block at a cell of the bot's current dimension, read without a call; `None` as for [`Host::hot`], and
    /// for a bot that tracks no terrain (no physics) or a chunk it has not loaded.
    pub fn block_at(&self, bot: &BotId, pos: BlockPos) -> Option<Block> {
        let world = self.registry.lock().reader(bot)?.world()?;
        convert::block(&world, pos)
    }

    /// Closes every bot (as `bot.close`, waiting at most 10 s for them), sends what is buffered and joins the shard
    /// threads. Later calls are refused `CLOSED`; the [`Outbound`] ends once drained.
    pub async fn shutdown(&self) {
        let emptied = self.registry.lock().close_all();
        if let Some(emptied) = emptied {
            let _ = tokio::time::timeout(Duration::from_secs(10), emptied.notified()).await;
        }
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
