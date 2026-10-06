use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::{oneshot, watch};

use super::events::{BotInfo, BotStatus};
use super::spec::{BotId, BotSpec, Target};

struct Entry<S> {
    target: Target,
    shard: usize,
    status: BotStatus,
    cancel: watch::Sender<bool>,
    /// Kept once the supervisor has stopped (failed), for `remove` to hand back.
    stopped: Option<BotSpec<S>>,
    /// A pending `remove`, answered when the supervisor exits.
    waiter: Option<oneshot::Sender<BotSpec<S>>>,
}

/// Every bot the swarm knows, running or failed.
pub(crate) struct Registry<S>(Mutex<HashMap<BotId, Entry<S>>>);

impl<S> Default for Registry<S> {
    fn default() -> Self {
        Self(Mutex::default())
    }
}

pub(crate) enum Take<S> {
    /// Signalled; the supervisor answers once it has disconnected and released the lease
    /// (dropped unanswered if the spec was lost to a panic).
    Pending(oneshot::Receiver<BotSpec<S>>),
    /// It had already failed; dropped from the table now.
    Stopped(Option<BotSpec<S>>),
    Unknown,
}

impl<S> Registry<S> {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<BotId, Entry<S>>> {
        self.0.lock().expect("swarm registry lock poisoned")
    }

    /// Lists a new bot on the first of `shards` running fewer than `fill` bots, or the least-loaded
    /// once all are that full (docs/swarm.md, "Shards"); `None` if the id is taken.
    pub fn insert(&self, id: &BotId, target: &Target, shards: usize, fill: usize) -> Option<(usize, watch::Receiver<bool>)> {
        let mut bots = self.lock();
        if bots.contains_key(id) {
            return None;
        }
        let load = load(&bots, shards);
        let shard = (0..shards).find(|&s| load[s] < fill).or_else(|| (0..shards).min_by_key(|&s| load[s])).unwrap_or(0);
        let (cancel, cancelled) = watch::channel(false);
        let entry = Entry { target: target.clone(), shard, status: BotStatus::Waiting, cancel, stopped: None, waiter: None };
        bots.insert(id.clone(), entry);
        Some((shard, cancelled))
    }

    pub fn set(&self, id: &BotId, status: BotStatus) {
        if let Some(e) = self.lock().get_mut(id) {
            e.status = status;
        }
    }

    /// Drops an entry whose supervisor exited, answering a pending `remove`.
    pub fn removed(&self, id: &BotId, spec: Option<BotSpec<S>>) {
        let waiter = self.lock().remove(id).and_then(|e| e.waiter);
        if let (Some(waiter), Some(spec)) = (waiter, spec) {
            let _ = waiter.send(spec);
        }
    }

    /// Marks the bot failed and keeps its spec; true if a pending `remove` took it instead.
    pub fn failed(&self, id: &BotId, error: String, spec: Option<BotSpec<S>>) -> bool {
        let mut bots = self.lock();
        let Some(e) = bots.get_mut(id) else { return false };
        if e.waiter.is_some() {
            drop(bots);
            self.removed(id, spec);
            return true;
        }
        e.status = BotStatus::Failed { error };
        e.stopped = spec;
        false
    }

    pub fn take(&self, id: &BotId) -> Take<S> {
        let mut bots = self.lock();
        match bots.get_mut(id) {
            None => Take::Unknown,
            Some(e) if matches!(e.status, BotStatus::Failed { .. }) => Take::Stopped(bots.remove(id).and_then(|e| e.stopped)),
            Some(e) => {
                let (tx, rx) = oneshot::channel();
                e.waiter = Some(tx);
                e.cancel.send_replace(true);
                Take::Pending(rx)
            }
        }
    }

    pub fn cancel_all(&self) {
        self.lock().values().for_each(|e| {
            e.cancel.send_replace(true);
        });
    }

    pub fn snapshot(&self, shards: usize) -> (Vec<BotInfo>, Vec<usize>) {
        let bots = self.lock();
        let info = bots
            .iter()
            .map(|(id, e)| BotInfo { id: id.clone(), target: e.target.clone(), shard: e.shard, status: e.status.clone() })
            .collect();
        (info, load(&bots, shards))
    }
}

/// Running bots per shard (failed ones cost nothing).
fn load<S>(bots: &HashMap<BotId, Entry<S>>, shards: usize) -> Vec<usize> {
    let mut load = vec![0; shards];
    for e in bots.values().filter(|e| !matches!(e.status, BotStatus::Failed { .. })) {
        load[e.shard] += 1;
    }
    load
}
