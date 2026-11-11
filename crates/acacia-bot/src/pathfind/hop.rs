//! Running and hopping: `Run` sprints up to three cells along a direction to arrive with its momentum
//! (to the edge of a gap, or one cell onto a lane of four or more); `Hop` sprint-jumps on along the
//! direction of a `Run` entry and lands wherever the jump comes down on the same level.

use std::f32::consts::SQRT_2;

use acacia_physics::BlockPos;

use super::cache::{Counts, Store, feet_key};
use super::control::RunCtl;
use super::costs::{self, HUMAN_JUMP};
use super::moves::offset;
use super::node::{DIRS, Entry, Momentum};
use super::profile::{Aim, MovementProfile};
use super::rollout::PlanSim;
use super::solve::{AIM_SEED, STRAIGHT, Solved, Solver};
use super::terrain::{Blocks, EPS, Spot, SpotKind, Terrain};
use crate::movement::Motion;

/// Farthest run (cells), reaching the edge of a gap.
const RUN_REACH: i32 = 3;
/// Flat cells ahead that make a lane worth running onto.
const LANE: i32 = 4;
/// A hop is aimed this far along its direction; it lands well short of it.
const HOP_AIM: i32 = 8;

#[derive(Clone)]
pub(crate) struct Ran {
    pub ctl: RunCtl,
    pub ticks: u32,
    pub end: Motion,
}

/// A dry floor at the start's level.
fn level<B: Blocks>(t: &Terrain<B>, q: BlockPos, from: Spot) -> Option<Spot> {
    t.spot(q).filter(|s| s.kind == SpotKind::Stand && !s.wet && (s.feet - from.feet).abs() < EPS)
}

/// The runs from `p`: along every direction from rest, along the running direction otherwise.
pub(crate) fn runs<B: Blocks, S: PlanSim<B>>(t: &Terrain<B>, p: BlockPos, from: Spot, (entry, mom): (Entry, &Momentum), start: &S, solver: &mut Solver<Terrain<B>>, (cache, n): (&Store, &mut Counts)) -> Vec<(BlockPos, Spot, Ran)> {
    let dirs = match entry {
        Entry::Run(d, _) => d..d + 1,
        _ => 0..DIRS.len() as u8,
    };
    let mut out = Vec::new();
    for d in dirs {
        if solver.spent() {
            break;
        }
        let (dx, dz) = DIRS[usize::from(d)];
        let at = |k: i32| offset(p, dx * k, 0, dz * k);
        let flat = (1..=LANE).take_while(|&k| level(t, at(k), from).is_some()).count() as i32;
        let gap = |q: BlockPos| t.spot(q).is_none() && t.spot(offset(q, 0, 1, 0)).is_none();
        // One cell onto a lane, or right up to the edge of a gap.
        let k = match flat {
            LANE => 1,
            1..=RUN_REACH if gap(at(flat + 1)) => flat,
            _ => continue,
        };
        let Some(to) = level(t, at(k), from) else { continue };
        let ran = cache.runs.get_or(t, p, (*mom, d, k, feet_key(to.feet, p)), n, || {
            let ctl = RunCtl::new(at(k), to.feet, Aim::new(solver.profile, AIM_SEED));
            let (bodies, end, clean) = solver.run(start, &mut { ctl }, false);
            clean.then(|| Ran { ctl, ticks: bodies.len() as u32 - 1, end })
        });
        if let Some(ran) = ran {
            out.push((at(k), to, ran));
        }
    }
    out
}

/// The hop on from running along direction `d`: where a sprint-jump along `d` lands on the same level, if it
/// lands there cleanly.
pub(crate) fn hop<B: Blocks, S: PlanSim<B>>(t: &Terrain<B>, p: BlockPos, from: Spot, d: u8, start: &S, solver: &mut Solver<Terrain<B>>) -> Option<(BlockPos, Spot, Solved)> {
    let (dx, dz) = DIRS[usize::from(d)];
    let (down, ticks) = solver.hop_landing(start, offset(p, dx * HOP_AIM, 0, dz * HOP_AIM), from.feet);
    let cell = [down.pos[0].floor() as i32, p[1], down.pos[2].floor() as i32];
    let to = level(t, cell, from).filter(|_| down.on_ground && cell != p)?;
    // `Human` drops hops no faster than sprinting there (see docs/pathfind-plan.md step 7, as built).
    let (a, b) = ((cell[0] - p[0]).abs() as f32, (cell[2] - p[2]).abs() as f32);
    if solver.profile == MovementProfile::Human && ticks as f32 + HUMAN_JUMP >= costs::sprint(S::envelope()) * (a.max(b) + (SQRT_2 - 1.0) * a.min(b)) {
        return None;
    }
    if let Some(s) = solver.hop(start, cell, to.feet) {
        return Some((cell, to, s));
    }
    // Came down too near the cell's edge for the profile's slack: coast into the cell before it.
    let back = offset(cell, -dx, 0, -dz);
    let to = level(t, back, from).filter(|_| back != p)?;
    Some((back, to, solver.solve(start, back, to.feet, false, STRAIGHT).ok()?))
}
