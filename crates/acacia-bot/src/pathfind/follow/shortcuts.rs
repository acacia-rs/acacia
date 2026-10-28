//! The follower's any-angle shortcuts (`pathfind/shortcut.rs`): when to look for one, and where the path ends.

use std::sync::Arc;

use acacia_physics::WorldView;

use super::Follower;
use crate::pathfind::hazard::Hazards;
use crate::cadence::splitmix;
use crate::movement::Rollout;
use crate::pathfind::rollout::Spend;
use crate::pathfind::shortcut;

impl Follower {
    /// On flat ground, run straight at nodes further on when that beats following the nodes between.
    pub fn shortcuts(mut self, on: bool) -> Self {
        self.shortcuts = on;
        self
    }

    /// The entities as they are now (once a tick, before [`Follower::tick`]).
    pub fn set_hazards(&mut self, hazards: Option<Arc<Hazards>>) {
        self.hazards = hazards;
    }

    /// Whether the path's last node is the goal (default) or a segment end that a planned-ahead segment continues.
    pub fn set_ends_at_goal(&mut self, yes: bool) {
        self.ends_at_goal = yes;
    }

    /// Once per node, with nothing running: start a shortcut to a node further on if one beats the nodes between.
    pub(super) fn try_shortcut(&mut self, live: Option<&Rollout>, w: &dyn WorldView, spend: &mut Spend) {
        let Some(live) = live.filter(|_| self.shortcuts) else { return };
        if self.macros.active() || self.tried.replace(self.index) == Some(self.index) {
            return;
        }
        let aim = self.aim.reseeded(splitmix(&mut self.rng));
        let hazards = self.hazards.as_deref();
        if let Some((ctl, predicted, last)) = shortcut::find(&self.path, self.index, live, w, aim, self.ends_at_goal, hazards, spend) {
            self.macros.begin(ctl, predicted, last);
        }
    }
}
