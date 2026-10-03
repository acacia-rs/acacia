//! Replies held back for vanilla's reaction times (vanilla mitm capture): the
//! client answers the login steps after tens to hundreds of ms, and pack and chunk-radius requests
//! after seconds. Packets leave in order, each pausing after the one before it.

use std::collections::VecDeque;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::time::{Duration, Instant};

use bytes::Bytes;

/// Delay ranges in ms, from the 2026-10-01 vanilla capture.
pub(super) mod delay {
    /// Vanilla 288 ms; signing the login takes part of it.
    pub const LOGIN: (u64, u64) = (220, 290);
    pub const HANDSHAKE: (u64, u64) = (30, 45);
    pub const CACHE_STATUS: (u64, u64) = (25, 40);
    pub const PACKS_HAVE_ALL: (u64, u64) = (30, 50);
    /// Last pack chunk → HaveAllPacks (vanilla 187 ms, one download, 2026-10-03).
    pub const PACKS_DOWNLOADED: (u64, u64) = (150, 230);
    pub const PACKS_COMPLETED: (u64, u64) = (650, 2800);
    pub const CHUNK_RADIUS: (u64, u64) = (1350, 1500);
    /// First ClientCacheBlobStatus after PlayerSpawn (vanilla 416 ms; blob-cache.md §3).
    pub const FIRST_BLOB_STATUS: (u64, u64) = (380, 450);
    /// Chunk data received → the client tick that reports its blobs (median 90 ms).
    pub const BLOB_STATUS: (u64, u64) = (40, 130);
}

pub(super) fn seed() -> u64 {
    RandomState::new().hash_one(0u8)
}

/// A uniform wait in `[lo, hi]` ms.
pub(super) fn wait(rng: &mut u64, (lo, hi): (u64, u64)) -> Duration {
    Duration::from_millis(lo + splitmix(rng) % (hi - lo + 1))
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

pub(super) struct Deferred {
    queue: VecDeque<(Instant, Bytes)>,
    rng: u64,
}

impl Deferred {
    pub fn new() -> Self {
        Self::with_seed(seed())
    }

    fn with_seed(rng: u64) -> Self {
        Self { queue: VecDeque::new(), rng }
    }

    pub fn push(&mut self, now: Instant, range: (u64, u64), packet: Bytes) {
        let wait = wait(&mut self.rng, range);
        // Replies to one burst of server packets follow one another, each after its own pause.
        let due = self.queue.back().map_or(now, |&(last, _)| now.max(last)) + wait;
        self.queue.push_back((due, packet));
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.queue.front().map(|&(due, _)| due)
    }

    pub fn pop_due(&mut self, now: Instant) -> Option<Bytes> {
        (self.next_due()? <= now).then(|| self.queue.pop_front().map(|(_, p)| p)).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_within_range_and_keeps_order() {
        let mut d = Deferred::with_seed(3);
        let t0 = Instant::now();
        d.push(t0, delay::PACKS_COMPLETED, Bytes::from_static(b"slow"));
        d.push(t0, delay::HANDSHAKE, Bytes::from_static(b"fast"));
        let due = d.next_due().unwrap() - t0;
        assert!((650..=2800).contains(&(due.as_millis() as u64)), "{due:?}");
        assert_eq!(d.pop_due(t0 + Duration::from_millis(600)), None);
        let late = t0 + Duration::from_secs(3);
        assert!(d.queue[1].0 >= d.queue[0].0 + Duration::from_millis(30), "the second pauses after the first");
        assert_eq!(d.pop_due(late).as_deref(), Some(&b"slow"[..]));
        assert_eq!(d.pop_due(late).as_deref(), Some(&b"fast"[..]), "a quicker reply never overtakes");
        assert_eq!(d.pop_due(late), None);
    }
}
