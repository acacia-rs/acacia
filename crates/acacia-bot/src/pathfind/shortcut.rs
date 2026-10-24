//! Any-angle shortcuts on flat ground (azalea's simulation executor): when the follower reaches a node, it
//! simulates sprinting (and sprint-jumping) straight at nodes further on and runs the fastest clean one
//! instead of the 8-direction staircase between them.

use acacia_physics::constants::DEFAULT_GROUND_DRAG;
use acacia_physics::{Aabb, BlockPos, WorldView};

use super::PathNode;
use super::control::{Controller, LAND_EPS, Phase, centre, standing_on, yaw_to};
use super::costs::{COST_HEURISTIC, MACRO_STARTUP};
use super::leap_moves::RUNNING;
use super::moves::MoveKind;
use super::profile::Aim;
use super::rollout::{Body, Spend};
use super::retry::live_hazard;
use super::solve::Solver;
use crate::movement::{Controls, Rollout};

/// Nodes ahead tried, farthest first.
const AHEAD: [usize; 7] = [20, 15, 10, 5, 4, 3, 2];
/// (jump at all, stop jumping this close to the target).
const STYLES: [(bool, f32); 3] = [(false, 0.0), (true, 1.0), (true, 3.0)];
/// Falling this far below the path means the line left the floor.
const DROP: f32 = 0.6;
/// Facing within this (degrees) of the held heading: keep it rather than re-aim.
const HELD_AIM_DEG: f32 = 1.0;
/// Landing up to this far (blocks) past the target's centre ends the shortcut there (a hop lands up to ~1.5 past).
const OVERSHOOT: f32 = 2.0;
/// Slower than this (blocks/tick), a shortcut has stopped on a target it may not overshoot.
const FINAL_SPEED: f32 = 0.05;
/// Ground coast per unit of speed, k / (1 − k).
const GROUND_COAST: f32 = DEFAULT_GROUND_DRAG / (1.0 - DEFAULT_GROUND_DRAG);
/// Flat nodes after the target a shortcut that may overshoot needs: one per block it may land past, and one more.
const OVERSHOOT_AHEAD: usize = 3;

/// Sprints straight at `target`, jumping whenever on the ground until within `stop_jump` blocks of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShortcutCtl {
    pub target: BlockPos,
    pub feet: f32,
    pub jump: bool,
    pub stop_jump: f32,
    pub aim: Aim,
    max: u16,
    t: u16,
    /// Bearing to the target, held once faced (per-tick re-aims draw strict BDS corrections, docs/DESIGN.md).
    heading: Option<f32>,
    origin: Option<[f32; 2]>,
    /// How far past the target a landing may end it: [`OVERSHOOT`] inside a flat span, else none.
    overshoot: f32,
    /// The target ends the path: stop on it.
    stop: bool,
}

impl ShortcutCtl {
    /// On the ground at the target's level, in its cell or past its centre near the line from the start: a hop
    /// that carried past the target is done there, not walked back (`Human` turned round on the 500-block lane).
    fn arrived(&self, b: &Body, origin: [f32; 2]) -> bool {
        // The path's end (no overshoot) is reached slowed down: arriving at a run, the follower circled back to it.
        if standing_on(b, self.target, self.feet) && (!self.stop || b.h_speed() < FINAL_SPEED) {
            return true;
        }
        let (past, aside) = self.past(b, origin);
        b.on_ground && (b.pos[1] - self.feet).abs() < LAND_EPS && (0.0..self.overshoot).contains(&past) && aside < 0.5
    }

    /// How far `b` is past the target's centre along the line from `origin`, and off that line.
    fn past(&self, b: &Body, origin: [f32; 2]) -> (f32, f32) {
        let [tx, tz] = centre(self.target);
        let (sx, sz) = (tx - origin[0], tz - origin[1]);
        let len = sx.hypot(sz).max(1e-3);
        let (dx, dz) = (b.pos[0] - tx, b.pos[2] - tz);
        ((dx * sx + dz * sz) / len, (dz * sx - dx * sz).abs() / len)
    }
}

impl Controller for ShortcutCtl {
    fn step(&mut self, b: &Body, _: &mut dyn FnMut(&Controls) -> Body, c: &mut Controls) -> Phase {
        let origin = *self.origin.get_or_insert([b.pos[0], b.pos[2]]);
        if self.arrived(b, origin) {
            return Phase::Done;
        }
        if self.t >= self.max || b.pos[1] < self.feet - DROP || (b.on_ground && (b.pos[1] - self.feet).abs() > LAND_EPS) {
            return Phase::Failed;
        }
        self.t += 1;
        let to = centre(self.target);
        (c.strafe, c.sneak, c.pitch, c.forward, c.sprint) = (0.0, false, 0.0, 1.0, true);
        // Re-aimed only while still turning onto it, and never once past a target it may overshoot (that turned round).
        let past = self.overshoot > 0.0 && self.past(b, origin).0 >= 0.0;
        let heading = match self.heading {
            Some(h) if ((b.yaw - h + 180.0).rem_euclid(360.0) - 180.0).abs() < HELD_AIM_DEG || past => h,
            _ => *self.heading.insert(yaw_to(b, to)),
        };
        c.yaw = self.aim.turn(b.yaw, heading);
        let dist = (b.pos[0] - to[0]).hypot(b.pos[2] - to[1]);
        c.jump = self.jump && b.on_ground && dist > self.stop_jump;
        // Braking for a stop-only target: let go once coasting on the ground covers what is left.
        if self.stop && b.on_ground && dist < b.h_speed() * GROUND_COAST {
            (c.forward, c.sprint, c.jump) = (0.0, false, false);
        }
        Phase::Going
    }
}

/// Flat ground at the level of `level`: no work, water or macro leap.
fn flat(n: &PathNode, level: f32) -> bool {
    matches!(n.kind, MoveKind::Walk | MoveKind::Run | MoveKind::Hop) && n.work.is_empty() && !n.wet && (n.feet - level).abs() < LAND_EPS
}

/// The shortcut from the live state to a node after `path[index]`: its controller, predicted bodies and the
/// index it lands on. Only taken when faster than following the nodes between; looked for within `spend`.
/// `ends_at_goal`: the path's last node is the goal, where a shortcut stops.
pub(super) fn find(path: &[PathNode], index: usize, live: &Rollout, w: &dyn WorldView, aim: Aim, ends_at_goal: bool, spend: &mut Spend) -> Option<(ShortcutCtl, Vec<Body>, usize)> {
    let level = live.st.pos[1];
    if !live.st.on_ground || !path.get(index).is_some_and(|n| flat(n, level)) {
        return None;
    }
    let reach = path[index..].iter().take_while(|n| flat(n, level)).count();
    let avoid = |p: BlockPos| live_hazard(w, p);
    let held = Controls { yaw: live.st.yaw, ..Controls::default() };
    let mut solver = Solver::new(w, &avoid, &|_| false, aim.profile, false, held);
    solver.budget = spend.left();
    // The span's last node first: a fixed step would stop short of it, and the rest is walked.
    let last = Some(reach - 1).filter(|&k| (2..=AHEAD[0]).contains(&k) && !AHEAD.contains(&k));
    let found = last.into_iter().chain(AHEAD).filter(|&k| k < reach).find_map(|k| {
        let j = index + k;
        let (along, planned) = planned(live, &path[index..=j]);
        // By distance, not node count: a hop node spans four blocks.
        let max = (4.0 * along) as u16 + 20;
        let best = STYLES
            .iter()
            .filter_map(|&(jump, stop_jump)| {
                // Only well inside the flat span, so the follower carries on to nodes still ahead: past a leap's
                // takeoff, the goal, or the span's last nodes it would have to come back.
                // Macro moves after it were planned from the target itself: a landing past it would start them elsewhere.
                let walked_on = k + OVERSHOOT_AHEAD < reach && path[j + 1..=j + OVERSHOOT_AHEAD].iter().all(|n| n.mac.is_none());
                let overshoot = if walked_on { OVERSHOOT } else { 0.0 };
                let ctl = ShortcutCtl { target: path[j].pos, feet: path[j].feet, jump, stop_jump, aim, max, t: 0, heading: None, origin: None, overshoot, stop: aim.profile.eased_stop() && ends_at_goal && j + 1 == path.len() };
                if !solver.affords(u64::from(max)) {
                    return None;
                }
                let (bodies, _, clean) = solver.run(live, &mut { ctl }, false);
                let centred = !aim.profile.centred_shortcuts() || bodies.iter().all(|b| !b.on_ground || over_floor(w, b));
                (clean && centred).then_some((ctl, bodies))
            })
            .min_by_key(|(_, b)| b.len());
        best.filter(|(_, b)| ((b.len() - 1) as f32) < planned).map(|(ctl, bodies)| (ctl, bodies, j))
    });
    spend.charge(&solver);
    found
}

/// A collision box right under the feet centre.
pub(crate) fn over_floor(w: &dyn WorldView, b: &Body) -> bool {
    let ([x, y, z], e) = (b.pos, 1e-3);
    let mut boxes = Vec::new();
    w.collisions(&Aabb::new(x - e, y - 0.01, z - e, x + e, y, z + e), &mut boxes);
    !boxes.is_empty()
}

/// Blocks from the live position through the nodes' centres, and the ticks following them takes: macro moves
/// as planned (simulated), walked segments at sprint speed plus the run-up when starting one from standing.
fn planned(live: &Rollout, nodes: &[PathNode]) -> (f32, f32) {
    let mut at = [live.st.pos[0], live.st.pos[2]];
    let standing = live.st.vel[0].hypot(live.st.vel[2]) < RUNNING;
    let (mut len, mut ticks) = (0.0, if standing && nodes[0].mac.is_none() { MACRO_STARTUP } else { 0.0 });
    for n in nodes {
        let c = centre(n.pos);
        let d = (c[0] - at[0]).hypot(c[1] - at[1]);
        len += d;
        ticks += if n.mac.is_some() { n.cost } else { d * COST_HEURISTIC };
        at = c;
    }
    (len, ticks)
}
