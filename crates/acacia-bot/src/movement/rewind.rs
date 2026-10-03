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
    /// Movement attribute values by the input tick they take effect on, oldest first: a rewind restores
    /// state saved before a late update arrived.
    speeds: VecDeque<(u64, f32)>,
}

impl History {
    pub(super) fn record(&mut self, tick: u64, input: Input, knockback: Option<Vec3>, after: &PlayerState) {
        if self.entries.len() == HISTORY {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { tick, input, knockback, after: after.clone() });
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

    /// Records a movement attribute stamped with server `tick`; like knockback, it applies from input tick + 1.
    pub(super) fn movement_attribute(&mut self, tick: u64, value: f32) {
        let oldest = self.entries.front().map_or(0, |e| e.tick);
        while self.speeds.len() > 1 && self.speeds[1].0 <= oldest {
            self.speeds.pop_front();
        }
        self.speeds.push_back((tick + 1, value));
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
        if let Some(v) = speed_at(&self.speeds, base) {
            st.set_movement_attribute(v);
        }
        for e in self.entries.iter_mut().filter(|e| e.tick > base) {
            if let Some(v) = speed_at(&self.speeds, e.tick).filter(|_| self.speeds.iter().any(|&(t, _)| t == e.tick)) {
                st.set_movement_attribute(v);
            }
            st.knockback = e.knockback;
            physics::tick(st, &e.input, world);
            e.after = st.clone();
            replayed += 1;
        }
        st.knockback = queued;
        replayed
    }
}

/// The newest movement attribute value in effect on input tick `tick`.
fn speed_at(speeds: &VecDeque<(u64, f32)>, tick: u64) -> Option<f32> {
    speeds.iter().rev().find(|&&(t, _)| t <= tick).map(|&(_, v)| v)
}
