//! Which leaps a node offers: landing cells around it (cardinal up to 7 blocks, the diagonals (2,1) (2,2)
//! (3,1) (3,2) (4,1)), each over at least one gap cell, through a corridor that is clear at jump height, onto
//! a dry floor one up to three down or into water deeper below.

use std::sync::LazyLock;

use acacia_physics::BlockPos;

use super::cache::{Counts, Store, feet_key};
use super::cell::Rect;
use super::moves::offset;
use super::node::Momentum;
use super::rollout::PlanSim;
use super::solve::{Miss, STRAIGHT, Shape, Solved, Solver, WIDE};
use super::terrain::{Blocks, Spot, SpotKind, Terrain};
use crate::movement::Sim;

/// Farthest cardinal leap (blocks to the landing cell): from rest a 4-gap needs a two-block run-up.
const MAX_CARDINAL: i32 = 7;
const DIAGONALS: [(i32, i32); 5] = [(2, 1), (2, 2), (3, 1), (3, 2), (4, 1)];
/// Dry landings from one up to three down.
const DRY_DY: [i32; 5] = [1, 0, -1, -2, -3];
/// Deepest drop scanned for a water landing.
const MAX_WATER_DROP: i32 = 16;
/// Widest gap a sprint-jump clears onto a floor no lower than the takeoff, without speed or jump effects
/// (`tests::leaps`: four lands, five never does).
const MAX_FLAT_GAP: usize = 4;
/// Horizontal speed (blocks per tick) from which a node counts as running (`node::speed_bin` 1 and up).
pub(super) const RUNNING: f32 = 0.12;

/// A landing column relative to the node and the cells the straight line to it crosses.
struct Offset {
    to: (i32, i32),
    /// `to` divided by its components' greatest common divisor: offsets on one line share it.
    unit: (i32, i32),
    crossed: Vec<(i32, i32)>,
    diagonal: bool,
}

fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// Every offset, nearest first, so a blocked cell is found before the targets it shadows.
static OFFSETS: LazyLock<Vec<Offset>> = LazyLock::new(|| {
    let mut tos: Vec<(i32, i32)> = (2..=MAX_CARDINAL).map(|k| (k, 0)).chain(DIAGONALS).collect();
    tos = tos.iter().flat_map(|&(a, b)| [(a, b), (b, a)]).flat_map(|(a, b)| [(a, b), (-a, b), (a, -b), (-a, -b)]).collect();
    tos.sort_by_key(|&(a, b)| (a * a + b * b, a, b));
    tos.dedup();
    tos.into_iter()
        .map(|to| {
            let g = gcd(to.0, to.1);
            Offset { to, unit: (to.0 / g, to.1 / g), crossed: crossed(to), diagonal: to.0 != 0 && to.1 != 0 }
        })
        .collect()
});

/// Cells (besides both ends) the segment between the two cell centres passes through.
fn crossed((a, b): (i32, i32)) -> Vec<(i32, i32)> {
    let steps = 16 * (a.abs() + b.abs());
    let mut out: Vec<(i32, i32)> = Vec::new();
    for i in 1..steps {
        let f = i as f32 / steps as f32;
        let c = ((0.5 + a as f32 * f).floor() as i32, (0.5 + b as f32 * f).floor() as i32);
        if c != (0, 0) && c != (a, b) && !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// Where a leap into column `c` can land: dry floors near the start's height, else the first water below.
fn landings<B: Blocks>(t: &Terrain<B>, c: BlockPos) -> Vec<(BlockPos, Spot)> {
    let dry = DRY_DY.iter().map(|&dy| offset(c, 0, dy, 0)).filter_map(|q| Some((q, t.spot(q).filter(|s| s.kind == SpotKind::Stand && !s.wet)?)));
    let mut out: Vec<_> = dry.collect();
    for dy in 1..=MAX_WATER_DROP {
        let q = offset(c, 0, -dy, 0);
        if let Some(s) = t.spot(q) {
            if s.wet && dy > 1 {
                out.push((q, s));
            }
            break;
        }
        if !t.passable(q) {
            break;
        }
    }
    out
}

/// A line of leaps from a node: unit step and landing dy.
pub(crate) type Line = ((i32, i32), i32);

/// A leap target from a node, not yet solved: the landing cell, its spot and the offset it lies at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LeapCand {
    pub q: BlockPos,
    pub to: Spot,
    off: u16,
}

impl LeapCand {
    /// The line (unit step, landing dy) it lies on: a nearer leap on it that fell short means this one does too.
    pub fn line(&self, p: BlockPos) -> Line {
        (OFFSETS[usize::from(self.off)].unit, self.q[1] - p[1])
    }

    /// Unit steps along its line: a leap that fell short makes only farther ones on the line fall short.
    pub fn steps(&self) -> i32 {
        let off = &OFFSETS[usize::from(self.off)];
        if off.unit.0 != 0 { off.to.0 / off.unit.0 } else { off.to.1 / off.unit.1 }
    }
}

/// The leap targets from `p` (standing at `from`, starting as `start`) that pass the geometric prefilters.
pub(crate) fn candidates<B: Blocks, S: Sim>(t: &Terrain<B>, p: BlockPos, from: Spot, start: &S) -> Vec<LeapCand> {
    let mut found = Vec::new();
    // Per crossed cell: no floor at the start's level; per cell and lane shape: body clear at takeoff and jump height.
    const W: i32 = 2 * MAX_CARDINAL + 1;
    let at = |(dx, dz): (i32, i32)| ((dx + MAX_CARDINAL) * W + dz + MAX_CARDINAL) as usize;
    let (mut gaps, mut clears) = ([None; (W * W) as usize], [[None; 2]; (W * W) as usize]);
    let mut gap = |c: (i32, i32)| {
        let q = offset(p, c.0, 0, c.1);
        *gaps[at(c)].get_or_insert_with(|| t.spot(q).is_none() && t.spot(offset(q, 0, 1, 0)).is_none())
    };
    let mut clear = |(dx, dz): (i32, i32), diagonal: bool| {
        let q = offset(p, dx, 0, dz);
        // Cardinal lanes are crossed at the body's width, diagonal ones anywhere in the cell.
        let rect = if diagonal { Rect::lane(1, 1) } else { Rect::lane(dx.signum(), dz.signum()) };
        *clears[at((dx, dz))][usize::from(diagonal)].get_or_insert_with(|| t.body_clear(q, rect, from.feet) && t.body_clear(q, rect, from.feet + 1.0))
    };
    let traits = start.traits();
    let plain = !traits.jump_boost && traits.speed <= S::envelope().movement_speed;
    let [vx, _, vz] = start.body().vel;
    let running = vx.hypot(vz) >= RUNNING;
    for (i, off) in OFFSETS.iter().enumerate() {
        // A runner leaps ahead or aside; a leap back starts from a stop, which the node standing here offers.
        let back = running && vx * off.to.0 as f32 + vz * off.to.1 as f32 <= 0.0;
        // Gaps first: open ground has none, and they are the cheaper probe.
        if back || !off.crossed.iter().any(|&c| gap(c)) || !off.crossed.iter().all(|&c| clear(c, off.diagonal)) {
            continue;
        }
        let too_wide = !off.diagonal && plain && widest_gap(&off.crossed, &mut gap) > MAX_FLAT_GAP;
        for (q, to) in landings(t, offset(p, off.to.0, 0, off.to.1)) {
            if !(too_wide && !to.wet && q[1] >= p[1]) {
                found.push(LeapCand { q, to, off: i as u16 });
            }
        }
    }
    found
}

/// The leap to `c` from `p`, solved from `start` (or the cache); `real_yaw`: from the momentum's facing, not turned to
/// the leap.
pub(crate) fn solve<B: Blocks, S: PlanSim<B>>(t: &Terrain<B>, p: BlockPos, (mom, real_yaw): (&Momentum, bool), start: &S, c: LeapCand, solver: &mut Solver<Terrain<B>>, (cache, n): (&Store, &mut Counts)) -> Result<Solved, Miss> {
    let (off, q, to) = (&OFFSETS[usize::from(c.off)], c.q, c.to);
    let key = (*mom, ((off.to.0, off.to.1, q[1] - p[1]), feet_key(to.feet, p), to.wet, real_yaw));
    cache.leaps.get_or(t, p, key, n, || {
        let start = if real_yaw { start.clone() } else { facing(start, off.to.0 as f32, off.to.1 as f32) };
        let straight = Shape { drop: to.wet || q[1] < p[1], ..STRAIGHT };
        let wide = Shape { drop: straight.drop, ..WIDE };
        let solved = solver.solve(&start, q, to.feet, to.wet, straight);
        // Turning off the line adds no reach: a leap that fell short straight falls short wide too.
        if off.diagonal && matches!(solved, Err(Miss::Other)) { solver.solve(&start, q, to.feet, to.wet, wide) } else { solved }
    })
}

/// Most gap cells in a row along `crossed`.
fn widest_gap(crossed: &[(i32, i32)], gap: &mut impl FnMut((i32, i32)) -> bool) -> usize {
    let (mut run, mut widest) = (0, 0);
    for &c in crossed {
        run = if gap(c) { run + 1 } else { 0 };
        widest = widest.max(run);
    }
    widest
}

/// `start` turned towards `(dx, dz)` if it stands still: a standing player faces where it goes next (a `Human`
/// aim eases from the facing, so it shapes the leap).
fn facing<S: Sim>(start: &S, dx: f32, dz: f32) -> S {
    let mut r = start.clone();
    let b = r.body();
    if b.vel[0] == 0.0 && b.vel[2] == 0.0 {
        r.set_motion(b.pos, b.vel, (-dx).atan2(dz).to_degrees());
    }
    r
}
