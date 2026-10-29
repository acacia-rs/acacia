//! When to stop trying: no progress towards the goal for too long, and backing off searches that find nothing.

use acacia_physics::Vec3;

use crate::pathfind::costs::COST_HEURISTIC;
use crate::pathfind::goal::Goal;
use crate::ActionError;

/// Wait after a search that found no way on, doubling each time up to `MAX_BACKOFF_TICKS`.
const BACKOFF_TICKS: u32 = 20;
const MAX_BACKOFF_TICKS: u32 = 160;

pub(super) struct Guard {
    tick: u32,
    /// Closest the bot came to the goal (heuristic ticks), and when.
    best: f32,
    best_at: u32,
    give_up: Option<u32>,
    wait_until: u32,
    backoff: u32,
    /// The last search started on the ground (an airborne one finds no start node: retry at once).
    on_ground: bool,
    pub searches: u32,
}

impl Guard {
    pub fn new(give_up: Option<u32>) -> Self {
        Self { tick: 0, best: f32::INFINITY, best_at: 0, give_up, wait_until: 0, backoff: BACKOFF_TICKS, on_ground: true, searches: 0 }
    }

    /// One tick with the bot at `pos`; fails once it has come no closer to `goal` (by a block) for `give_up` ticks.
    pub fn tick(&mut self, goal: &Goal, pos: Vec3) -> Result<(), ActionError> {
        self.tick += 1;
        let h = goal.heuristic([pos[0].floor() as i32, (pos[1] + 0.01).floor() as i32, pos[2].floor() as i32]);
        if h < self.best - COST_HEURISTIC {
            (self.best, self.best_at) = (h, self.tick);
        } else if self.give_up.is_some_and(|t| self.tick - self.best_at > t) {
            return Err(ActionError::NotPossible(format!("cannot reach {goal:?}: no closer for {} ticks", self.tick - self.best_at)));
        }
        Ok(())
    }

    pub fn searched(&mut self, on_ground: bool) {
        self.searches += 1;
        self.on_ground = on_ground;
    }

    pub fn back_off(&mut self) {
        if self.on_ground {
            self.wait_until = self.tick + self.backoff;
            self.backoff = (self.backoff * 2).min(MAX_BACKOFF_TICKS);
        }
    }

    /// [`Self::back_off`] in the air too, `extra` ticks longer: the way is closed by an entity, not a bad start.
    pub fn wait_out(&mut self, extra: u32) {
        self.wait_until = self.tick + self.backoff + extra;
        self.backoff = (self.backoff * 2).min(MAX_BACKOFF_TICKS);
    }

    pub fn found(&mut self) {
        self.backoff = BACKOFF_TICKS;
    }

    pub fn waiting(&self) -> bool {
        self.tick < self.wait_until
    }
}
