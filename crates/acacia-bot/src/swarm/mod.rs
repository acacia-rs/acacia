//! Many bots, many servers, one process: each bot's task runs on one of K single-threaded shards,
//! and a supervisor reconnects it per a [`Policy`]. Specs, events and snapshots are plain serde
//! data, so a coordinator can place bots across nodes through this handle. Design: docs/swarm.md.

mod builder;
mod events;
mod join_queue;
mod keeper;
mod lease;
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
pub use lease::{AccountLease, Lease, LeaseError, LocalLeases};
pub use policy::{Decision, DisconnectHook, Policy};
pub use spec::{BotId, BotSpec, Login, Target};

use registry::Take;
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
    shard_fill: usize,
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

    /// Queues the bot for its first join, on a shard chosen as [`SwarmBuilder::shard_fill`] describes.
    pub fn add(&self, spec: BotSpec<S>) -> Result<(), AddError> {
        let ctx = &self.inner.ctx;
        if ctx.draining.load(Ordering::Relaxed) {
            return Err(AddError::Draining);
        }
        let id = spec.id.clone();
        let placed = ctx.registry.insert(&id, &spec.target, self.inner.shard_count, self.inner.shard_fill);
        let (shard, cancel) = placed.ok_or_else(|| AddError::Duplicate(id.clone()))?;
        let job = (self.inner.spawn)(spec, cancel);
        let shards = self.inner.shards.lock().expect("swarm shards lock poisoned");
        if !shards.get(shard).is_some_and(|s| s.submit(id.clone(), job)) {
            ctx.registry.removed(&id, None);
            return Err(AddError::ShutDown);
        }
        Ok(())
    }

    /// Disconnects the bot (or forgets a failed one) and returns its spec with the current state,
    /// once it is offline and its account lease is released: a move is `other.add(remove().await?)`.
    /// `None` if the id is unknown, or the bot ended (its task returned, or it panicked) first.
    pub async fn remove(&self, id: &BotId) -> Option<BotSpec<S>> {
        match self.inner.ctx.registry.take(id) {
            Take::Pending(spec) => spec.await.ok(),
            Take::Stopped(spec) => {
                self.inner.ctx.emit(SwarmEvent::Removed { id: id.clone() });
                spec
            }
            Take::Unknown => None,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let (bots, shard_load) = self.inner.ctx.registry.snapshot(self.inner.shard_count);
        Snapshot { bots, shard_load, draining: self.inner.ctx.draining.load(Ordering::Relaxed) }
    }

    pub fn events(&self) -> broadcast::Receiver<SwarmEvent> {
        self.inner.ctx.events.subscribe()
    }

    /// No more joins or reconnects, for handing a node's bots over before it goes away: waiting
    /// bots park (`BotStatus::Parked`), online ones stay until removed or disconnected (then park).
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
