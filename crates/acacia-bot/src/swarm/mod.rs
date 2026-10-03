//! Many bots, many servers, one process: each bot's task runs on one of K single-threaded shards,
//! and a supervisor reconnects it per a [`Policy`]. Specs, events and snapshots are plain serde
//! data, so a coordinator can place bots across nodes through this handle. Design: docs/swarm.md.

mod builder;
mod events;
mod join_queue;
mod login;
mod policy;
mod registry;
mod shard;
mod spec;
mod supervisor;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, watch};

pub use builder::SwarmBuilder;
pub use events::{BotInfo, BotStatus, Snapshot, SwarmEvent};
pub use policy::{Decision, DisconnectHook, Policy};
pub use spec::{BotId, BotSpec, Login, Target};

use registry::Cancelled;
use shard::{Job, Shard};
use supervisor::Ctx;

type Spawn<S> = dyn Fn(BotSpec<S>, watch::Receiver<bool>) -> Job + Send + Sync;

/// A cloneable handle to a running swarm.
pub struct Swarm<S> {
    inner: Arc<Inner<S>>,
}

struct Inner<S> {
    ctx: Arc<Ctx<S>>,
    spawn: Box<Spawn<S>>,
    shards: Mutex<Vec<Shard>>,
    shard_count: usize,
}

/// Dropping the last handle disconnects every bot rather than leaving them unreachable.
impl<S> Drop for Inner<S> {
    fn drop(&mut self) {
        self.ctx.registry.cancel_all();
    }
}

impl<S> Clone for Swarm<S> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AddError {
    #[error("bot {0} is already in the swarm")]
    Duplicate(BotId),
    #[error("the swarm is draining")]
    Draining,
    #[error("the swarm has shut down")]
    ShutDown,
}

impl<S: Send + 'static> Swarm<S> {
    pub fn builder() -> SwarmBuilder<S> {
        SwarmBuilder::default()
    }

    /// Queues the bot for its first join on the least-loaded shard.
    pub fn add(&self, spec: BotSpec<S>) -> Result<(), AddError> {
        let ctx = &self.inner.ctx;
        if ctx.draining.load(Ordering::Relaxed) {
            return Err(AddError::Draining);
        }
        let id = spec.id.clone();
        let (shard, cancel) = ctx.registry.insert(&id, &spec.target, self.inner.shard_count).ok_or_else(|| AddError::Duplicate(id.clone()))?;
        let job = (self.inner.spawn)(spec, cancel);
        let shards = self.inner.shards.lock().expect("swarm shards lock poisoned");
        if !shards.get(shard).is_some_and(|s| s.submit(id.clone(), job)) {
            ctx.registry.remove(&id);
            return Err(AddError::ShutDown);
        }
        Ok(())
    }

    /// Disconnects the bot (or forgets a failed one). False if the id is unknown.
    pub fn remove(&self, id: &BotId) -> bool {
        match self.inner.ctx.registry.cancel(id) {
            Cancelled::Signalled => true,
            Cancelled::Dropped => {
                self.inner.ctx.emit(SwarmEvent::Removed { id: id.clone() });
                true
            }
            Cancelled::Unknown => false,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let (bots, shard_load) = self.inner.ctx.registry.snapshot(self.inner.shard_count);
        Snapshot { bots, shard_load, draining: self.inner.ctx.draining.load(Ordering::Relaxed) }
    }

    pub fn events(&self) -> broadcast::Receiver<SwarmEvent> {
        self.inner.ctx.events.subscribe()
    }

    /// No more joins or reconnects: waiting bots leave, online bots stay until they disconnect or
    /// are removed. For handing a node's bots over before it goes away.
    pub fn drain(&self) {
        self.inner.ctx.draining.store(true, Ordering::Relaxed);
    }

    /// Drains, disconnects every bot and waits for the shard threads to exit.
    pub async fn shutdown(&self) {
        self.drain();
        // Taken before cancelling: a racing add() either submitted already (and is cancelled
        // below) or finds no shards.
        let shards = std::mem::take(&mut *self.inner.shards.lock().expect("swarm shards lock poisoned"));
        self.inner.ctx.registry.cancel_all();
        for shard in shards {
            shard.stop().await;
        }
    }
}
