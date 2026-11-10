//! Macro-move results kept across nodes, searches and re-plans: a result is stored relative to its node
//! with every cell its rollouts read, and reused at any node whose momentum and family match and whose
//! cells read the same (failures too). One cache is shared by every search of a goto, concurrent ones
//! included (a plan-ahead and a re-plan): each family's map is split into locked shards, held only for a
//! lookup or an insert, never while a result is computed.

use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault, Hash};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use std::any::Any;

use acacia_physics::BlockPos;

use super::hop::Ran;
use super::node::Momentum;
use super::profile::MovementProfile;
use super::shift::{Shift, add};
use super::solve::{Miss, Solved};
use super::terrain::{Blocks, PosHasher, Spot, Terrain};
use crate::movement::Sim;

/// Results kept per key; more means the same move from the same momentum met that many surroundings.
const PER_KEY: usize = 32;
/// Results a cache keeps across searches before starting over (each holds its read cells, a few KB).
const MAX_RESULTS: usize = 50_000;
/// Locked shards per move family (a power of two).
const SHARDS: usize = 16;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Entry<V> {
    /// Cells read, relative to the node, with their ids.
    reads: Box<[(BlockPos, u64)]>,
    value: V,
}

type Shard<K, V> = HashMap<K, Vec<Entry<V>>, BuildHasherDefault<PosHasher>>;

/// One move family's results.
pub(crate) struct Memo<K, V> {
    shards: [Mutex<Shard<K, V>>; SHARDS],
    len: AtomicUsize,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Self { shards: std::array::from_fn(|_| Mutex::default()), len: AtomicUsize::new(0) }
    }
}

impl<K: Hash + Eq + Clone, V: Shift + Clone> Memo<K, V> {
    fn shard(&self, key: &K) -> MutexGuard<'_, Shard<K, V>> {
        let h = BuildHasherDefault::<PosHasher>::default().hash_one(key);
        lock(&self.shards[(h >> 60) as usize % SHARDS])
    }

    /// The result for `key` at node `p`: a stored one whose cells all read the same here, else `compute`'s.
    pub fn get_or<B: Blocks>(&self, t: &Terrain<B>, p: BlockPos, key: K, n: &mut Counts, compute: impl FnOnce() -> V) -> V {
        if n.off {
            n.misses += 1;
            return compute();
        }
        {
            let mut shard = self.shard(&key);
            let entries = shard.entry(key.clone()).or_default();
            for i in 0..entries.len() {
                let reads = &mut entries[i].reads;
                match reads.iter().position(|&(q, id)| t.cell(add(p, q)).id() != id) {
                    // The cell that told this entry apart goes first: neighbouring nodes tend to differ there too.
                    Some(j) => reads.swap(0, j),
                    None => {
                        n.hits += 1;
                        // Most recently used first: a full key drops its last.
                        entries[..=i].rotate_right(1);
                        let mut v = entries[0].value.clone();
                        v.shift(p);
                        return v;
                    }
                }
            }
        }
        n.misses += 1;
        let (v, reads) = t.record(compute);
        let mut stored = v.clone();
        let back = p.map(|c| -c);
        stored.shift(back);
        let reads = reads.into_iter().map(|(q, id)| (add(q, back), id)).collect();
        let mut shard = self.shard(&key);
        let entries = shard.entry(key).or_default();
        if entries.len() == PER_KEY {
            entries.pop();
            n.evicted += 1;
        } else {
            self.len.fetch_add(1, Ordering::Relaxed);
        }
        entries.insert(0, Entry { reads, value: stored });
        v
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub hits: u32,
    pub misses: u32,
    /// Results dropped for a full key.
    pub evicted: u32,
    /// Simulate everything and keep nothing (comparisons in tests).
    pub off: bool,
}

/// Leap key beyond the momentum: landing offset, feet height relative to the node (1/1024), into water, from the
/// momentum's own facing.
pub(crate) type LeapKey = ((i32, i32, i32), i16, bool, bool);

/// A hop's landing cell, its spot and the leap.
pub(crate) type Hopped = (BlockPos, Spot, Solved);

/// The results for one movement profile and player.
#[derive(Default)]
pub(crate) struct Store {
    /// The player the results hold for: its simulation at rest at the origin.
    owner: Option<(MovementProfile, Box<dyn Any + Send + Sync>)>,
    pub off: bool,
    pub runs: Memo<(Momentum, u8, i32, i16), Option<Ran>>,
    pub hops: Memo<(Momentum, u8), Option<Hopped>>,
    pub leaps: Memo<(Momentum, LeapKey), Result<Solved, Miss>>,
}

impl Store {
    fn len(&self) -> usize {
        [&self.runs.len, &self.hops.len, &self.leaps.len].iter().map(|l| l.load(Ordering::Relaxed)).sum()
    }
}

/// Macro-move results for one movement profile and player (attributes, effects, equipment); a
/// [`Pilot`](super::Pilot) shares one across its searches, running ones included.
#[derive(Default)]
pub struct RolloutCache {
    store: Mutex<Arc<Store>>,
    off: bool,
}

impl RolloutCache {
    /// The results for `profile` and the player `base`: the held ones, or a fresh store if they were found for
    /// another player or the cache is full (a search still using the old store keeps it).
    pub(crate) fn claim<S: Sim>(&self, profile: MovementProfile, base: &S) -> Arc<Store> {
        let player = Momentum::default().synth([0, 0, 0], base);
        let mut store = lock(&self.store);
        let held = store.owner.as_ref().is_some_and(|(p, s)| *p == profile && s.downcast_ref::<S>() == Some(&player));
        if !held || store.len() > MAX_RESULTS {
            *store = Arc::new(Store { owner: Some((profile, Box::new(player))), off: self.off, ..Store::default() });
        }
        store.clone()
    }

    /// A cache that keeps nothing: every search simulates every move.
    pub fn disabled() -> Self {
        Self { off: true, ..Self::default() }
    }
}

/// Feet height relative to node `p` in `Momentum` steps, for keys.
pub(crate) fn feet_key(feet: f32, p: BlockPos) -> i16 {
    ((feet - p[1] as f32) * 1024.0).round() as i16
}
