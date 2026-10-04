//! Which peers have datagrams to send or a timer due, so the server never has to ask all of them.
//!
//! `ready` is a round-robin queue of peers that may have output. `timers` is a min-heap of
//! wake-ups that are never updated or removed: a peer's timeout mostly moves later (each ACK
//! pushes its resend time back), and an entry that fires early just leads to a re-arm. A new
//! entry is added only when the timeout moves before the peer's earliest one, so a peer's
//! entries are exactly `Peer::wakes`, a stack with the earliest on top and a handful deep.
//! Entries that match no stack top belong to peers that left and are skipped.
//!
//! Invariant: every peer is in `ready` or has an entry at or before its real timeout.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::net::SocketAddr;
use std::time::Instant;

use super::Peer;

#[derive(Default)]
pub(super) struct Schedule {
    ready: VecDeque<SocketAddr>,
    timers: BinaryHeap<Reverse<(Instant, SocketAddr)>>,
}

impl Schedule {
    /// For a peer whose state changed: it may have output, and its timeout may be earlier.
    pub fn touch(&mut self, addr: SocketAddr, peer: &mut Peer) {
        self.mark_ready(addr, peer);
        self.arm(addr, peer);
    }

    pub fn mark_ready(&mut self, addr: SocketAddr, peer: &mut Peer) {
        if !std::mem::replace(&mut peer.ready, true) {
            self.ready.push_back(addr);
        }
    }

    pub fn arm(&mut self, addr: SocketAddr, peer: &mut Peer) {
        let at = peer.poll_timeout();
        if peer.wakes.last().is_none_or(|&earliest| at < earliest) {
            peer.wakes.push(at);
            self.timers.push(Reverse((at, addr)));
        }
    }

    /// The next address to poll for output; it may belong to a peer that left.
    pub fn pop_ready(&mut self) -> Option<SocketAddr> {
        self.ready.pop_front()
    }

    pub fn next_timeout(&self) -> Option<Instant> {
        self.timers.peek().map(|Reverse((at, _))| *at)
    }

    /// Removes and returns the wake-ups due by `now`, stale ones included.
    pub fn take_due(&mut self, now: Instant) -> Vec<(Instant, SocketAddr)> {
        let mut due = Vec::new();
        while self.timers.peek().is_some_and(|Reverse((at, _))| *at <= now) {
            due.push(self.timers.pop().expect("peeked").0);
        }
        due
    }

    #[cfg(test)]
    pub fn timers(&self) -> usize {
        self.timers.len()
    }
}
