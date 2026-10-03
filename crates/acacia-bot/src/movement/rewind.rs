//! Rewind and replay, as the vanilla client does for server-authoritative movement: a correction,
//! teleport or knockback stamped with an input tick we have already simulated past changes the
//! state as of that tick, and the inputs sent since are simulated again on top of it.

use std::collections::VecDeque;

use acacia_physics::{self as physics, Input, PlayerState, Vec3, WorldView};

/// Ticks kept for replay (5 s): older events apply as of now.
const HISTORY: usize = 100;

#[derive(Debug, Clone, Copy)]
pub(super) struct Correction {
    pub feet: Vec3,
    pub delta: Vec3,
    pub on_ground: bool,
    /// A teleport target: BDS holds the player there for the tick but keeps applying liquid currents.
    pub teleport: bool,
}

struct Entry {
    tick: u64,
    input: Input,
    knockback: Option<Vec3>,
    /// A server movement attribute (without sprint and freeze) taking effect on this tick.
    movement: Option<f32>,
    /// The server's freeze count after this tick: its newest attribute stamped here, which includes server ticks
    /// that ran without an input since.
    frozen: Option<u32>,
    /// The state at the end of the tick.
    after: PlayerState,
}

#[derive(Default)]
pub(super) struct History {
    entries: VecDeque<Entry>,
    pending: Option<(u64, Correction)>,
    newest: u64,
    /// Earliest tick given a knockback since the last replay.
    knocked: Option<u64>,
}

impl History {
    pub(super) fn record(&mut self, tick: u64, input: Input, knockback: Option<Vec3>, after: &PlayerState) {
        if self.entries.len() == HISTORY {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { tick, input, knockback, movement: None, frozen: None, after: after.clone() });
    }

    /// A server movement attribute effective from input `tick`: kept tick states from then on take it, and a
    /// replay over `tick` applies it there, so rewinding to an older state doesn't undo it. Our predicted freeze
    /// counts from then on shift by the server's correction; returns the newest kept count, if `tick` is kept.
    pub(super) fn movement_attribute(&mut self, tick: u64, base: f32, frozen: u32) -> Option<u32> {
        let at = self.entries.iter().position(|e| e.tick == tick)?;
        let shift = i64::from(frozen) - i64::from(self.entries[at].after.frozen_ticks);
        for e in self.entries.iter_mut().skip(at) {
            if e.tick == tick {
                e.movement = Some(base);
                e.frozen = Some(frozen);
            }
            e.after.set_movement_attribute(base);
            e.after.set_frozen_ticks((i64::from(e.after.frozen_ticks) + shift).max(0) as u32);
        }
        self.entries.back().map(|e| e.after.frozen_ticks)
    }

    /// Resets the state as of the end of `tick` before the next simulated tick. The newest tick wins:
    /// a lagging server can send a teleport after corrections that already include it.
    pub(super) fn schedule(&mut self, tick: u64, correction: Correction) {
        if tick < self.newest {
            return;
        }
        self.newest = tick;
        self.pending = Some((tick, correction));
    }

    /// Applies a knockback on already simulated input `tick`; false when that tick (or the one before,
    /// to start from) is no longer kept.
    pub(super) fn knockback(&mut self, tick: u64, velocity: Vec3) -> bool {
        if !self.entries.iter().any(|e| e.tick + 1 == tick) {
            return false;
        }
        let Some(e) = self.entries.iter_mut().find(|e| e.tick == tick) else { return false };
        e.knockback = Some(velocity);
        self.knocked = Some(self.knocked.map_or(tick, |k| k.min(tick)));
        true
    }

    /// Rewinds to a scheduled correction, or to before the earliest new knockback, and replays the
    /// kept ticks after it. Returns the ticks replayed.
    pub(super) fn apply(&mut self, st: &mut PlayerState, world: &impl WorldView) -> usize {
        let knocked = self.knocked.take();
        let queued = st.knockback;
        let base = if let Some((tick, c)) = self.pending.take() {
            // The rest of the state (jump delay, sprint, swim...) must be as of `tick`, not as of now;
            // a teleport still queued for the next tick stays queued.
            if let Some(e) = self.entries.iter().find(|e| e.tick == tick) {
                let teleport = st.pending_teleport;
                *st = e.after.clone();
                st.pending_teleport = teleport;
            }
            st.apply_correction(c.feet, c.delta, c.on_ground);
            if c.teleport {
                physics::apply_current(st, world);
            }
            tick
        } else if let Some(k) = knocked
            && let Some(e) = self.entries.iter().find(|e| e.tick + 1 == k)
        {
            *st = e.after.clone();
            e.tick
        } else {
            return 0;
        };
        let mut replayed = 0;
        if self.entries.front().is_some_and(|e| e.tick > base + 1) {
            st.knockback = queued;
            return replayed;
        }
        for e in self.entries.iter_mut().filter(|e| e.tick > base) {
            if let Some(m) = e.movement {
                st.set_movement_attribute(m);
            }
            st.knockback = e.knockback;
            physics::tick(st, &e.input, world);
            if let Some(f) = e.frozen {
                st.set_frozen_ticks(f);
            }
            e.after = st.clone();
            replayed += 1;
        }
        st.knockback = queued;
        replayed
    }
}
