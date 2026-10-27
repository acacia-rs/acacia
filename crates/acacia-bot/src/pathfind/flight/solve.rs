//! The pitch a glide holds next: each candidate is held for a short rollout and scored on progress, height and
//! clearance (Baritone's pitch solve by simulation, docs/research/pathfinder-survey.md). Re-solved every tick, the
//! held pitches add up to a pitch schedule.

use acacia_physics::{Vec3, WorldView};

use super::surface::Surface;
use crate::movement::{Controls, Rollout};

/// Ticks a cruise candidate is held: about 45 blocks at cruise speed, past what a rocket changes.
pub(super) const CRUISE_TICKS: u32 = 30;
/// Ticks a landing candidate may take to touch down.
pub(super) const LANDING_TICKS: u32 = 110;
/// Ticks simulated after touchdown for the slide to stop.
const SLIDE_TICKS: u32 = 30;

const COARSE: [f32; 11] = [-60.0, -40.0, -25.0, -12.0, -5.0, 0.0, 5.0, 12.0, 25.0, 40.0, 60.0];
const FINE: [f32; 4] = [-4.0, -2.0, 2.0, 4.0];

/// Cost weights, in blocks of progress: a block of height off the wanted line costs as much as 1.5 blocks not flown.
const HEIGHT_W: f32 = 1.5;
const CLEAR_W: f32 = 20.0;
const WALL: f32 = 2000.0;
const GROUND: f32 = 1000.0;
/// Per tick left when the rollout reaches terrain not yet known.
const UNKNOWN: f32 = 20.0;
const REST_W: f32 = 6.0;
/// Clearance kept over terrain that rises above the wanted line.
const MIN_CLEAR: f32 = 2.0;
/// Fall distance a touchdown may carry: damage starts past 3, and the live glide may sink a little faster.
const SAFE_FALL: f32 = 2.0;

/// The line a glide is flown along: cruise height, then down a slope onto the landing spot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Line {
    /// Feet of the landing spot (or of the target, before one is found).
    pub target: Vec3,
    pub cruise_y: f32,
    /// Height lost per horizontal block coming in.
    pub slope: f32,
    /// Clearance kept over the terrain while cruising.
    pub margin: f32,
    /// Score touchdowns: the rollouts run to where the slide stops.
    pub landing: bool,
}

/// Feet height above the landing spot where the slope starts flattening into the touchdown.
const FLARE: f32 = 1.0;

impl Line {
    pub fn height_at(&self, d: f32) -> f32 {
        self.cruise_y.min(self.target[1] + FLARE + d * self.slope)
    }

    fn dist(&self, p: Vec3) -> f32 {
        (p[0] - self.target[0]).hypot(p[2] - self.target[2])
    }
}

/// The best pitch to hold from `r` facing `yaw`, near `prev` for an equal score.
pub(super) fn solve(r: &Rollout, yaw: f32, prev: f32, line: &Line, w: &(impl WorldView + ?Sized), s: &mut dyn Surface) -> f32 {
    let mut cost = |pitch: f32| score(r, yaw, pitch, line, w, s) + (pitch - prev).abs() * 0.01;
    let (mut best, mut best_cost) = (prev, f32::INFINITY);
    for p in COARSE.into_iter().chain([prev]) {
        let c = cost(p);
        if c < best_cost {
            (best, best_cost) = (p, c);
        }
    }
    let coarse = best;
    for p in FINE.map(|d| (coarse + d).clamp(-90.0, 90.0)) {
        let c = cost(p);
        if c < best_cost {
            (best, best_cost) = (p, c);
        }
    }
    best
}

/// Cost of holding `pitch` (degrees) facing `yaw` from `r`.
pub(super) fn score(r: &Rollout, yaw: f32, pitch: f32, line: &Line, w: &(impl WorldView + ?Sized), s: &mut dyn Surface) -> f32 {
    let c = Controls { forward: 1.0, glide: true, yaw, pitch, ..Controls::default() };
    let mut f = r.clone();
    let d0 = line.dist(f.st.pos);
    let ticks = if line.landing { LANDING_TICKS } else { CRUISE_TICKS };
    let mut cost = 0.0;
    for t in 0..ticks {
        let fall = f.st.fall_distance;
        f.step(&c, w);
        let p = f.st.pos;
        // A hit charged by how soon it comes: from low down every candidate hits, and the latest is the way out.
        let soon = (ticks - t) as f32 / ticks as f32;
        if f.st.collide_x || f.st.collide_z {
            return cost + WALL * soon;
        }
        if f.st.on_ground {
            if !line.landing {
                return cost + GROUND * soon;
            }
            let damage = (fall - SAFE_FALL).max(0.0);
            return cost + REST_W * line.dist(slide(f, w)) + damage * 100.0;
        }
        let Some(top) = s.top(p[0].floor() as i32, p[2].floor() as i32) else {
            return cost + UNKNOWN * (ticks - t) as f32 + HEIGHT_W * (p[1] - line.height_at(line.dist(p))).abs();
        };
        // The clearance the wanted line itself has there (descending), and none at the spot, where the floor is the landing.
        let floor = (top + 1) as f32;
        let d = line.dist(p);
        let need = if line.landing && d < 8.0 { 0.0 } else { line.margin.min((line.height_at(d) - floor).max(MIN_CLEAR)) };
        let clear = p[1] - floor;
        if clear < need {
            cost += (need - clear) * CLEAR_W;
        }
    }
    let end = f.st.pos;
    let d1 = line.dist(end);
    cost - (d0 - d1) + HEIGHT_W * (end[1] - line.height_at(d1)).abs() + if line.landing { REST_W * d1 } else { 0.0 }
}

/// Where the body stops after touching down with nothing held.
fn slide(mut f: Rollout, w: &(impl WorldView + ?Sized)) -> Vec3 {
    let idle = Controls { yaw: f.st.yaw, ..Controls::default() };
    for _ in 0..SLIDE_TICKS {
        f.step(&idle, w);
    }
    f.st.pos
}

/// Furthest horizontal distance an unpowered glide from `r` covers before its feet drop to `floor_y`.
pub(super) fn glide_reach(r: &Rollout, yaw: f32, floor_y: f32, w: &(impl WorldView + ?Sized)) -> f32 {
    const MAX_TICKS: u32 = 20 * 120;
    [0.0f32, 5.0, 10.0, 15.0, 20.0, 30.0]
        .into_iter()
        .map(|pitch| {
            let c = Controls { forward: 1.0, glide: true, yaw, pitch, ..Controls::default() };
            let mut f = r.clone();
            f.st.glide_boost_ticks = 0;
            let start = f.st.pos;
            for _ in 0..MAX_TICKS {
                f.step(&c, w);
                if f.st.on_ground || f.st.pos[1] <= floor_y || !f.st.gliding {
                    break;
                }
            }
            (f.st.pos[0] - start[0]).hypot(f.st.pos[2] - start[2])
        })
        .fold(0.0, f32::max)
}
