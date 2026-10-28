//! What the pilot reports: its goal, settings and counters.

use std::sync::Arc;

use super::Pilot;
use crate::pathfind::cache::RolloutCache;
use crate::pathfind::follow::Follower;
use crate::pathfind::goal::Goal;
use crate::pathfind::goto::GotoOpts;

impl Pilot {
    pub fn cache(&self) -> &Arc<RolloutCache> {
        &self.cache
    }

    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    pub fn opts(&self) -> &GotoOpts {
        &self.opts
    }

    pub fn replans(&self) -> u32 {
        self.replans
    }

    pub fn divergences(&self) -> u32 {
        self.divergences
    }

    /// Re-plans started by an entity moving onto the path ahead.
    pub fn threat_replans(&self) -> u32 {
        self.alarm.replans
    }

    pub fn is_partial(&self) -> bool {
        self.partial
    }

    pub fn follower(&self) -> Option<&Follower> {
        self.follower.as_ref()
    }

    /// Searches started (inline ones included).
    pub fn searches(&self) -> u32 {
        self.guard.searches
    }

    /// The last node of each segment a planned-ahead segment was spliced onto, in order.
    pub fn joins(&self) -> &[acacia_physics::BlockPos] {
        &self.ahead.joins
    }
}
