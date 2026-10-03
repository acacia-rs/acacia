use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::watch;

use super::events::{BotInfo, BotStatus};
use super::spec::{BotId, Target};

struct Entry {
    target: Target,
    shard: usize,
    status: BotStatus,
    cancel: watch::Sender<bool>,
}

/// Every bot the swarm knows, running or failed.
#[derive(Default)]
pub(crate) struct Registry(Mutex<HashMap<BotId, Entry>>);

pub(crate) enum Cancelled {
    /// The supervisor will disconnect it and report `Removed`.
    Signalled,
    /// It had already failed; dropped from the table now.
    Dropped,
    Unknown,
}

impl Registry {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<BotId, Entry>> {
        self.0.lock().expect("swarm registry lock poisoned")
    }

    /// Lists a new bot on the least-loaded of `shards`; `None` if the id is taken.
    pub fn insert(&self, id: &BotId, target: &Target, shards: usize) -> Option<(usize, watch::Receiver<bool>)> {
        let mut bots = self.lock();
        if bots.contains_key(id) {
            return None;
        }
        let load = load(&bots, shards);
        let shard = (0..shards).min_by_key(|&s| load[s]).unwrap_or(0);
        let (cancel, cancelled) = watch::channel(false);
        bots.insert(id.clone(), Entry { target: target.clone(), shard, status: BotStatus::Waiting, cancel });
        Some((shard, cancelled))
    }

    pub fn set(&self, id: &BotId, status: BotStatus) {
        if let Some(e) = self.lock().get_mut(id) {
            e.status = status;
        }
    }

    pub fn remove(&self, id: &BotId) {
        self.lock().remove(id);
    }

    pub fn cancel(&self, id: &BotId) -> Cancelled {
        let mut bots = self.lock();
        match bots.get(id) {
            None => Cancelled::Unknown,
            Some(e) if matches!(e.status, BotStatus::Failed { .. }) => {
                bots.remove(id);
                Cancelled::Dropped
            }
            Some(e) => {
                e.cancel.send_replace(true);
                Cancelled::Signalled
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
fn load(bots: &HashMap<BotId, Entry>, shards: usize) -> Vec<usize> {
    let mut load = vec![0; shards];
    for e in bots.values().filter(|e| !matches!(e.status, BotStatus::Failed { .. })) {
        load[e.shard] += 1;
    }
    load
}
