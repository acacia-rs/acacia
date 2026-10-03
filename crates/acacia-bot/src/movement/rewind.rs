//! Rewind and replay, as the vanilla client does for server-authoritative movement: a correction,
//! teleport or knockback stamped with an input tick we have already simulated past changes the
//! state as of that tick, and the inputs sent since are simulated again on top of it.

use std::collections::VecDeque;

use acacia_physics::{self as physics, Effects, Input, PlayerState, Vec3, WorldView};

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
    speeds: Timeline<f32>,
    effects: Timeline<Effects>,
    /// The server's freeze as of the end of each tick (see `PlayerState::server_freeze`).
    freezes: Timeline<f32>,
}

/// Server-sent values by the input tick they take effect on, oldest first: a rewind restores state saved
/// before a late update arrived.
struct Timeline<T>(VecDeque<(u64, T)>);

impl<T> Default for Timeline<T> {
    fn default() -> Self {
        Self(VecDeque::new())
    }
}

impl<T: Copy> Timeline<T> {
    /// Records a value stamped with server `tick`; like knockback, it applies from input tick + 1.
    fn push(&mut self, tick: u64, value: T, oldest_kept: u64) {
        while self.0.len() > 1 && self.0[1].0 <= oldest_kept {
            self.0.pop_front();
        }
        self.0.push_back((tick + 1, value));
    }

    /// The newest value in effect on input tick `tick`.
    fn at(&self, tick: u64) -> Option<T> {
        self.0.iter().rev().find(|&&(t, _)| t <= tick).map(|&(_, v)| v)
    }

    /// [`Self::at`] when a value starts on exactly `tick`.
    fn starting(&self, tick: u64) -> Option<T> {
        self.at(tick).filter(|_| self.0.iter().any(|&(t, _)| t == tick))
    }
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

    pub(super) fn movement_attribute(&mut self, tick: u64, value: f32) {
        self.speeds.push(tick, value, self.entries.front().map_or(0, |e| e.tick));
    }

    /// Records the server's freeze stamped with `tick`; returns what we simulated for the tick it belongs to,
    /// if that tick is kept.
    pub(super) fn freeze(&mut self, tick: u64, freeze: f32) -> Option<f32> {
        self.freezes.push(tick, freeze, self.entries.front().map_or(0, |e| e.tick));
        self.entries.iter().find(|e| e.tick == tick + 1).map(|e| e.after.freeze)
    }

    pub(super) fn effects(&mut self, tick: u64, effects: Effects) {
        self.effects.push(tick, effects, self.entries.front().map_or(0, |e| e.tick));
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
        if let Some(v) = self.speeds.at(base) {
            st.set_movement_attribute(v);
        }
        if let Some(v) = self.effects.at(base) {
            st.effects = v;
        }
        if let Some(v) = self.freezes.starting(base) {
            st.set_freeze(v);
        }
        for e in self.entries.iter_mut().filter(|e| e.tick > base) {
            if let Some(v) = self.speeds.starting(e.tick) {
                st.set_movement_attribute(v);
            }
            if let Some(v) = self.effects.starting(e.tick) {
                st.effects = v;
            }
            st.server_freeze = self.freezes.starting(e.tick);
            st.knockback = e.knockback;
            physics::tick(st, &e.input, world);
            e.after = st.clone();
            replayed += 1;
        }
        st.knockback = queued;
        replayed
    }
}
