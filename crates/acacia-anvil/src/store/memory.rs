//! The memory bound: evicting written columns, and reading them back from their region files.

use std::collections::HashSet;
use std::time::Instant;

use super::disk::decode;
use super::stats::lap;
use super::write::hash_bytes;
use super::{AnvilStore, Entry, Pos, region_of};
use crate::Error;

fn around((x, z): Pos) -> impl Iterator<Item = Pos> {
    (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| (x + dx, z + dz)))
}

impl AnvilStore {
    /// Brings back from disk the stale columns (to be relit) and every neighbour a dirty column lacks.
    pub(super) fn reload(&mut self) -> Result<(), Error> {
        let start = Instant::now();
        let stale = std::mem::take(&mut self.stale);
        self.load(stale.keys().copied().collect())?;
        for (p, since) in stale {
            if self.columns.contains_key(&p) {
                self.dirty.entry(p).or_insert(since);
            }
        }
        let missing: Vec<Pos> = self
            .dirty
            .keys()
            .flat_map(|&p| around(p))
            .filter(|p| !self.columns.contains_key(p))
            .collect();
        self.load(missing)?;
        lap(&mut self.stats.read_back, start);
        Ok(())
    }

    /// Reads back those of `positions` that are on disk and not in memory. The files are read region by
    /// region (the disk keeps one open); decompressing and decoding run on all workers.
    fn load(&mut self, mut positions: Vec<Pos>) -> Result<(), Error> {
        positions.sort_unstable_by_key(|&p| (region_of(p), p));
        positions.dedup();
        let mut payloads = Vec::new();
        for p in positions {
            if !self.columns.contains_key(&p)
                && let Some(payload) = self.disk.payload(p)?
            {
                payloads.push((p, payload));
            }
        }
        let start = Instant::now();
        let per = payloads.len().div_ceil(self.workers.len()).max(1);
        let decoded: Vec<_> = std::thread::scope(|s| {
            let jobs: Vec<_> = payloads
                .chunks(per)
                .map(|part| s.spawn(move || part.iter().map(|(p, b)| (*p, decode(*p, b))).collect::<Vec<_>>()))
                .collect();
            jobs.into_iter()
                .flat_map(|j| j.join().expect("read-back worker panicked"))
                .collect()
        });
        lap(&mut self.stats.read_back_decode, start);
        for (pos, col) in decoded {
            let Some((col, nbt)) = col else { continue };
            self.stats.read_back_columns += 1;
            self.bytes += col.heap_bytes();
            let entry = Entry {
                col,
                converted: Instant::now(),
                written: Some(hash_bytes(&nbt)),
                fixed: true,
            };
            self.columns.insert(pos, entry);
        }
        Ok(())
    }

    /// Drops the oldest written columns above the memory bound; dirty ones and their neighbours stay.
    pub(super) fn evict(&mut self) {
        if self.bytes <= self.cfg.memory_limit {
            return;
        }
        let start = Instant::now();
        let keep: HashSet<Pos> = self.dirty.keys().flat_map(|&p| around(p)).collect();
        let mut old: Vec<(Pos, Instant)> = self
            .columns
            .iter()
            .filter(|(p, _)| !keep.contains(p))
            .map(|(p, e)| (*p, e.converted))
            .collect();
        old.sort_by_key(|&(_, t)| t);
        for (pos, _) in old {
            if self.bytes <= self.cfg.memory_limit * 9 / 10 {
                break;
            }
            if let Some(e) = self.columns.remove(&pos) {
                self.bytes -= e.col.heap_bytes();
            }
        }
        lap(&mut self.stats.evict, start);
    }
}
