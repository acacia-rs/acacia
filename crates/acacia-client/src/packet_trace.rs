//! Inbound packet ring for stall diagnostics: the last [`RING`] packet ids with arrival times,
//! recorded before the subscription filter drops anything. Ids only, never decoded.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use crate::proto::packet_name;

pub const RING: usize = 256;
pub const TAIL: usize = 20;

/// What [`crate::Client::packet_trace`] reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PacketTraceSummary {
    /// Milliseconds since the last inbound packet; `None` before the first.
    pub silent_ms: Option<u64>,
    /// Inbound packets since the connection started, filtered ones included.
    pub total: u64,
    /// Packet names over the ring (the last [`RING`] packets).
    pub counts_by_name: BTreeMap<String, u32>,
    /// The last [`TAIL`] packets, oldest first, as `Name@-12ms`.
    pub tail: Vec<String>,
}

#[derive(Default)]
struct Ring {
    entries: Vec<(u32, Instant)>,
    total: u64,
}

impl Ring {
    fn push(&mut self, id: u32, at: Instant) {
        let slot = (self.total % RING as u64) as usize;
        if self.entries.len() < RING {
            self.entries.push((id, at));
        } else {
            self.entries[slot] = (id, at);
        }
        self.total += 1;
    }

    /// Oldest first.
    fn ordered(&self) -> impl Iterator<Item = &(u32, Instant)> {
        let split = (self.total % RING as u64) as usize;
        let (newer, older) = self.entries.split_at(split.min(self.entries.len()));
        older.iter().chain(newer)
    }
}

/// Shared between the connection task (writer) and the [`crate::Client`] (reader).
#[derive(Clone, Default)]
pub(crate) struct PacketTrace(Arc<Mutex<Ring>>);

impl PacketTrace {
    /// One lock for a pass's packets: a lock each was 2.6% of a redstone swarm.
    pub(crate) fn record(&self, ids: &[u32], at: Instant) {
        let mut ring = self.lock();
        ids.iter().for_each(|&id| ring.push(id, at));
    }

    pub(crate) fn summary(&self) -> PacketTraceSummary {
        self.summary_at(Instant::now())
    }

    fn summary_at(&self, now: Instant) -> PacketTraceSummary {
        let ring = self.lock();
        let ms = |at: Instant| now.saturating_duration_since(at).as_millis() as u64;
        let mut counts_by_name = BTreeMap::new();
        for &(id, _) in &ring.entries {
            *counts_by_name.entry(name(id)).or_insert(0) += 1;
        }
        let skip = ring.entries.len().saturating_sub(TAIL);
        let tail = ring.ordered().skip(skip).map(|&(id, at)| format!("{}@-{}ms", name(id), ms(at))).collect();
        let silent_ms = ring.ordered().last().map(|&(_, at)| ms(at));
        PacketTraceSummary { silent_ms, total: ring.total, counts_by_name, tail }
    }

    fn lock(&self) -> MutexGuard<'_, Ring> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl crate::Client {
    /// Recent inbound packets and how long the server has been silent, for stall diagnostics.
    pub fn packet_trace(&self) -> PacketTraceSummary {
        self.trace.summary()
    }
}

fn name(id: u32) -> String {
    packet_name(id).map_or_else(|| format!("Unknown{id}"), str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::proto::packets::{SetTime, Text};
    use crate::proto::Packet;

    use super::*;

    #[test]
    fn empty_trace_has_no_silence() {
        assert_eq!(PacketTrace::default().summary(), PacketTraceSummary::default());
    }

    #[test]
    fn ring_keeps_the_newest_and_reports_silence() {
        let trace = PacketTrace::default();
        let t0 = Instant::now();
        for i in 0..300u64 {
            let id = if i % 2 == 0 { SetTime::ID } else { Text::ID };
            trace.record(&[id], t0 + Duration::from_millis(i));
        }
        trace.record(&[9999], t0 + Duration::from_millis(300));
        let s = trace.summary_at(t0 + Duration::from_millis(350));
        assert_eq!((s.total, s.silent_ms), (301, Some(50)));
        assert_eq!(s.counts_by_name.values().sum::<u32>(), RING as u32);
        assert_eq!(s.counts_by_name["Unknown9999"], 1);
        assert_eq!((s.counts_by_name["SetTime"], s.counts_by_name["Text"]), (127, 128));
        assert_eq!(s.tail.len(), TAIL);
        assert_eq!(s.tail.last().map(String::as_str), Some("Unknown9999@-50ms"));
        assert_eq!(s.tail[TAIL - 2], "Text@-51ms");
    }
}
