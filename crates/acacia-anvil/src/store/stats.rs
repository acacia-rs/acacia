//! Running totals of where a store's time and bytes go (`AnvilStore::stats`), for benches and logs.

use std::time::{Duration, Instant};

#[derive(Default, Debug, Clone)]
pub struct StoreStats {
    /// Evicted columns read back from region files (as neighbours or to be relit), and the time it took.
    pub read_back: Duration,
    pub read_back_columns: u64,
    /// The part of `read_back` decompressing and decoding chunks (on all workers; the rest is file reads).
    pub read_back_decode: Duration,
    /// Whole region files read before a rewrite.
    pub region_load: Duration,
    pub region_load_bytes: u64,
    pub fixup: Duration,
    /// Wall time of the parallel light + encode + compress phase.
    pub render_wall: Duration,
    /// Summed over worker threads.
    pub light: Duration,
    pub encode: Duration,
    pub compress: Duration,
    pub save: Duration,
    pub region_writes: u64,
    pub bytes_written: u64,
    /// Chunks whose slot was replaced, and relit chunks left as they were (same NBT).
    pub chunks_written: u64,
    pub chunks_unchanged: u64,
    pub evict: Duration,
}

/// Map sizes of a store's bookkeeping, to spot what grows with the explored area.
#[derive(Debug, Clone, Copy)]
pub struct Census {
    pub columns: usize,
    pub dirty: usize,
    pub stale: usize,
    pub regions_known: usize,
    pub regions_debounced: usize,
}

/// Per-worker times, folded into [`StoreStats`] after each region.
#[derive(Default)]
pub(super) struct WorkerTimes {
    pub light: Duration,
    pub encode: Duration,
    pub compress: Duration,
}

impl StoreStats {
    pub(super) fn add_worker(&mut self, t: &mut WorkerTimes) {
        let t = std::mem::take(t);
        self.light += t.light;
        self.encode += t.encode;
        self.compress += t.compress;
    }
}

/// Adds the time since `start` to `acc` and returns now.
pub(super) fn lap(acc: &mut Duration, start: Instant) -> Instant {
    let now = Instant::now();
    *acc += now - start;
    now
}
