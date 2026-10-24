//! Classic steering: the controls that walk, jump and sprint towards the current node.

use acacia_physics::constants::DEFAULT_GROUND_DRAG as GROUND_DRAG;
use acacia_physics::{Vec3, WorldView};

use super::{Follower, SETTLED, Sense, center, dir, hdist};
use crate::movement::{Controls, Rollout};
use crate::pathfind::PathNode;
use crate::pathfind::moves::MoveKind;
use crate::pathfind::rollout::Spend;

/// A coast must end this far inside the arrival tolerance.
const SETTLE_MARGIN: f32 = 0.05;
/// Ticks a coast is simulated for at most.
const COAST_TICKS: u32 = 40;
/// Straight nodes ahead before sprinting pays off.
const SPRINT_RUN: usize = 3;
/// Facing farther than this (degrees) off the node, stop walking until turned: an eased aim lagging the bearing
/// walks an arc (it circled the goal, and walked off the course's stones pit turning round).
const TURN_FIRST_DEG: f32 = 30.0;

impl Follower {
    #[allow(clippy::too_many_arguments)]
    /// `coast`: on the final node, where releasing every key now comes to rest (simulated).
    pub(super) fn steer(&mut self, s: &Sense, c: &mut Controls, node: &PathNode, target: Vec3, dist: f32, speed: f32, last: bool, coast: Option<Vec3>) {
        let (dx, dz) = (target[0] - s.pos[0], target[2] - s.pos[2]);
        let want = (-dx).atan2(dz).to_degrees();
        if dist > 0.05 {
            c.yaw = self.aim.turn(c.yaw, want);
        }
        let aim_off = ((want - c.yaw + 180.0).rem_euclid(360.0) - 180.0).abs();
        c.strafe = 0.0;
        c.sneak = false;
        // Over the node (ladders, pillars) the bearing is noise and the aim holds.
        c.forward = if aim_off > TURN_FIRST_DEG && (last || dist > 0.05) {
            0.0
        } else if last {
            // Ground friction lets the bot coast: release the key once coasting ends within the tolerance.
            let stops = match coast {
                Some(end) => hdist(end, target) < self.tolerance - SETTLE_MARGIN,
                None => dist < speed * GROUND_DRAG / (1.0 - GROUND_DRAG) + 0.05,
            };
            if stops { 0.0 } else { 1.0 }
        } else if matches!(node.kind, MoveKind::ClimbUp | MoveKind::ClimbDown | MoveKind::Pillar | MoveKind::Down) && dist < 0.15 {
            0.0
        } else {
            1.0
        };
        let rise = node.feet - s.pos[1];
        c.jump = match node.kind {
            MoveKind::Ascend => rise > 0.3 && dist < 1.3,
            MoveKind::ClimbUp => true,
            MoveKind::Parkour => s.on_ground && self.along(s.pos) > 0.35,
            _ if s.in_water => rise > -0.3,
            _ => false,
        };
        let wanted = !s.in_water
            && match node.kind {
                MoveKind::Parkour | MoveKind::Run | MoveKind::Hop => true,
                MoveKind::Walk => self.straight_run() >= SPRINT_RUN || self.runs_into_macro(),
                _ => false,
            };
        // The sprint key only starts a sprint; Movement keeps it going until forward is released.
        c.sprint = wanted;
    }

    /// Where `live` comes to rest with every key released, if the budget allows simulating it.
    pub(super) fn coast_end(live: &Rollout, c: &Controls, w: &dyn WorldView, spend: &mut Spend) -> Option<Vec3> {
        if !spend.affords(u64::from(COAST_TICKS)) {
            return None;
        }
        let (mut r, mut keys) = (live.clone(), *c);
        keys.stop();
        for _ in 0..COAST_TICKS {
            let before = r.st.pos;
            r.step(&keys, w);
            spend.ticks += 1;
            if r.st.on_ground && hdist(before, r.st.pos) < SETTLED {
                return Some(r.st.pos);
            }
        }
        None
    }

    /// Distance travelled from the segment origin along the segment direction.
    fn along(&self, pos: Vec3) -> f32 {
        let t = center(&self.path[self.index]);
        let (sx, sz) = (t[0] - self.origin[0], t[2] - self.origin[2]);
        let len = sx.hypot(sz).max(1e-3);
        ((pos[0] - self.origin[0]) * sx + (pos[2] - self.origin[2]) * sz) / len
    }

    /// Consecutive walk nodes from the current one heading the same way.
    fn straight_run(&self) -> usize {
        let mut prev = self.origin;
        let heading = dir(prev, center(&self.path[self.index]));
        let mut run = 0;
        for n in &self.path[self.index..] {
            if n.kind != MoveKind::Walk || !n.work.is_empty() || dir(prev, center(n)) != heading {
                break;
            }
            prev = center(n);
            run += 1;
        }
        run
    }

    /// The next node is a macro move heading on the way the bot walks now: arriving at speed is a run-up.
    fn runs_into_macro(&self) -> bool {
        let Some(next) = self.path.get(self.index + 1).filter(|n| n.mac.is_some()) else { return false };
        let here = center(&self.path[self.index]);
        let unit = |(x, z): (i32, i32)| (x.signum(), z.signum());
        unit(dir(self.origin, here)) == unit(dir(here, center(next)))
    }
}
