//! Re-plans when an entity moves onto the path ahead: the hazard score of the next nodes now, against the lowest it
//! has been since the path was accepted (the path was planned round what was there then).

use acacia_physics::Vec3;

use crate::pathfind::PathNode;
use crate::pathfind::follow::Follower;
use crate::pathfind::hazard::Hazards;

/// Nodes ahead whose hazard score is watched.
const AHEAD: usize = 8;
/// Score (ticks) over the baseline that re-plans.
const RISE: f32 = 6.0;
/// A cell a solid entity fills, on the path ahead.
const CLOSED: f32 = 1000.0;
/// Ticks between threat re-plans: one the new plan cannot avoid does not re-plan every tick.
const COOLDOWN: u32 = 10;

#[derive(Debug, Default)]
pub(super) struct Alarm {
    baseline: Option<f32>,
    quiet: u32,
    pub replans: u32,
}

impl Alarm {
    /// A new path: its first score becomes the baseline.
    pub fn accepted(&mut self) {
        self.baseline = None;
    }

    /// Once per tick while following; true: re-plan now.
    pub fn tick(&mut self, hazards: Option<&Hazards>, f: &Follower, pos: Vec3) -> bool {
        self.quiet = self.quiet.saturating_sub(1);
        let danger = hazards.map_or(0.0, |h| score(h, f, pos));
        let base = *self.baseline.get_or_insert(danger);
        if danger > base + RISE && self.quiet == 0 {
            (self.quiet, self.baseline) = (COOLDOWN, None);
            self.replans += 1;
            return true;
        }
        self.baseline = Some(base.min(danger));
        false
    }
}

/// The hazard score of the nodes `f` has ahead of `pos`: from the one nearest it, since a running shortcut holds the
/// follower's index at its start.
fn score(h: &Hazards, f: &Follower, pos: Vec3) -> f32 {
    let ahead = f.ahead();
    let d = |n: &PathNode| (n.pos[0] as f32 + 0.5 - pos[0]).powi(2) + (n.pos[2] as f32 + 0.5 - pos[2]).powi(2);
    let from = (0..ahead.len()).min_by(|&a, &b| d(&ahead[a]).total_cmp(&d(&ahead[b]))).unwrap_or(0);
    let cell = |n: &PathNode| if h.closed(n.pos) || h.closed([n.pos[0], n.pos[1] + 1, n.pos[2]]) { CLOSED } else { h.cost(n.pos) };
    ahead[from..].iter().take(AHEAD).map(cell).sum()
}
