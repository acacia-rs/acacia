//! Successor moves of a node (azalea `moves/basic.rs` + `parkour.rs`, plus climbing and swimming);
//! moves that dig, build or open doors are in [`super::alter`].

use std::f32::consts::SQRT_2;

use acacia_physics::BlockPos;
use acacia_physics::constants::STEP_HEIGHT;

use super::alter;
use super::cell::{CLIMB as CLIMBABLE, Rect};
use super::costs::{self, CENTER_AFTER_FALL, CLIMB, DANGER_NEAR, ENTER_WATER, JUMP_PENALTY, LONG_JUMP_PENALTY, SPRINT, SWIM, WALK_OFF};
use super::kit::Kit;
use super::terrain::{Blocks, EPS, Spot, SpotKind, Terrain};
use super::work::Work;

/// How a path node is reached from the previous one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    Walk,
    /// Needs a jump (rise above the step height).
    Ascend,
    /// Walk off an edge and fall.
    Descend,
    /// Sprint-jump over a one- or two-block gap.
    Parkour,
    ClimbUp,
    ClimbDown,
    Swim,
    /// Jump straight up and place a block underneath.
    Pillar,
    /// Break the block underneath and drop onto the one below it.
    Down,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Edge {
    pub pos: BlockPos,
    pub spot: Spot,
    pub kind: MoveKind,
    pub cost: f32,
    pub work: Work,
}

/// What the search may do besides walking, per expanded node.
pub(crate) struct Ctx<'k> {
    pub parkour: bool,
    pub dig: bool,
    pub bridge: bool,
    pub doors: bool,
    pub kit: &'k Kit,
    /// Scaffold blocks not yet used by the path to this node.
    pub blocks_left: u32,
}

const DIRS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
/// Highest feet rise a standing jump clears (apex ≈ 1.25 with the 0.42 jump velocity).
const MAX_JUMP_RISE: f32 = 1.2;
/// Falls up to 3 blocks do no damage.
const MAX_SAFE_FALL: f32 = 3.0;
/// Deepest fall scanned for a water landing.
const MAX_SCAN: i32 = 24;

pub(crate) fn offset(p: BlockPos, dx: i32, dy: i32, dz: i32) -> BlockPos {
    [p[0] + dx, p[1] + dy, p[2] + dz]
}

pub(crate) fn successors<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, ctx: &Ctx, out: &mut Vec<Edge>) {
    for (dx, dz) in DIRS {
        let start = out.len();
        let side = offset(p, dx, 0, dz);
        let level = t.spot(side);
        if let Some(to) = level {
            horizontal(t, p, from, side, to, (dx, dz), out);
        }
        let up = offset(side, 0, 1, 0);
        if let Some(to) = t.spot(up) {
            horizontal(t, p, from, up, to, (dx, dz), out);
        } else if level.is_none() && (dx == 0 || dz == 0) {
            descend(t, p, from, side, (dx, dz), out);
            if ctx.parkour && from.kind == SpotKind::Stand && !from.wet {
                gap_jump(t, p, from, (dx, dz), out);
            }
        }
        if dx == 0 || dz == 0 {
            alter::forward(t, p, from, (dx, dz), ctx, start, out);
        }
    }
    vertical(t, p, from, out);
    alter::vertical(t, p, from, ctx, out);
}

fn horizontal<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, to_pos: BlockPos, to: Spot, (dx, dz): (i32, i32), out: &mut Vec<Edge>) {
    let diag = dx != 0 && dz != 0;
    let rise = to.feet - from.feet;
    let jump = rise > STEP_HEIGHT + EPS;
    if rise > MAX_JUMP_RISE || (jump && diag) {
        return;
    }
    let hi = from.feet.max(to.feet);
    if jump && !t.body_clear_from(p, from, Rect::FOOT, to.feet) {
        return;
    }
    if !t.body_clear_from(p, from, Rect::toward(dx, dz), hi) || !t.body_clear(to_pos, Rect::toward(-dx, -dz), hi) {
        return;
    }
    if diag
        && !(t.body_clear(offset(p, dx, 0, 0), Rect::corner(-dx, dz), hi)
            && t.body_clear(offset(p, 0, 0, dz), Rect::corner(dx, -dz), hi))
    {
        return;
    }
    let wet = from.wet || to.wet;
    let mut cost = if diag { SQRT_2 } else { 1.0 } * if wet { SWIM } else { SPRINT };
    if !from.wet && to.wet {
        cost += ENTER_WATER;
    }
    let kind = if jump {
        cost = cost.max(costs::ascend());
        MoveKind::Ascend
    } else if wet {
        MoveKind::Swim
    } else {
        MoveKind::Walk
    };
    push(t, out, to_pos, to, kind, cost, Work::default());
}

fn descend<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, side: BlockPos, (dx, dz): (i32, i32), out: &mut Vec<Edge>) {
    if !t.body_clear_from(p, from, Rect::toward(dx, dz), from.feet) || !t.body_clear(side, Rect::toward(-dx, -dz), from.feet) {
        return;
    }
    for k in 1..=MAX_SCAN {
        let q = offset(side, 0, -k, 0);
        if let Some(to) = t.spot(q) {
            let fall = from.feet - to.feet;
            if fall > MAX_SAFE_FALL + EPS && !to.wet {
                return;
            }
            let mut cost = WALK_OFF + costs::fall_ticks(fall).max(CENTER_AFTER_FALL);
            if to.wet && !from.wet {
                cost += ENTER_WATER;
            }
            return push(t, out, q, to, MoveKind::Descend, cost, Work::default());
        }
        if !t.passable(q) {
            return;
        }
    }
}

/// Parkour over one or two gap cells onto a floor at about the same height.
fn gap_jump<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, (dx, dz): (i32, i32), out: &mut Vec<Edge>) {
    let lift = from.feet + 1.0;
    if !t.body_clear_from(p, from, Rect::toward(dx, dz), lift) {
        return;
    }
    for gap in 1..=2 {
        let g = offset(p, dx * gap, 0, dz * gap);
        let lane = Rect::lane(dx, dz);
        if gap > 1 && (t.spot(g).is_some() || t.spot(offset(g, 0, 1, 0)).is_some()) {
            return;
        }
        if !t.body_clear(g, lane, from.feet) || !t.body_clear(g, lane, lift) {
            return;
        }
        let land = offset(g, dx, 0, dz);
        if let Some(to) = t.spot(land).filter(|s| s.kind == SpotKind::Stand && !s.wet)
            && (to.feet - from.feet).abs() <= 0.5 + EPS
            && t.body_clear(land, Rect::toward(-dx, -dz), from.feet.max(to.feet))
        {
            let cost = SPRINT * (gap + 1) as f32 + JUMP_PENALTY + LONG_JUMP_PENALTY * (gap - 1) as f32;
            return push(t, out, land, to, MoveKind::Parkour, cost, Work::default());
        }
    }
}

fn vertical<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, out: &mut Vec<Edge>) {
    let (up, down) = (offset(p, 0, 1, 0), offset(p, 0, -1, 0));
    let none = Work::default();
    if t.cell(p).has(CLIMBABLE)
        && let Some(to) = t.spot(up)
    {
        push(t, out, up, to, MoveKind::ClimbUp, CLIMB, none);
    }
    if t.cell(down).has(CLIMBABLE)
        && let Some(to) = t.spot(down)
    {
        push(t, out, down, to, MoveKind::ClimbDown, CLIMB, none);
    }
    if from.wet {
        if let Some(to) = t.spot(up).filter(|s| s.kind == SpotKind::Swim) {
            push(t, out, up, to, MoveKind::Swim, SWIM, none);
        }
        if let Some(to) = t.spot(down).filter(|s| s.wet) {
            push(t, out, down, to, MoveKind::Swim, SWIM, none);
        }
    }
}

pub(crate) fn push<B: Blocks>(t: &Terrain<B>, out: &mut Vec<Edge>, pos: BlockPos, spot: Spot, kind: MoveKind, mut cost: f32, work: Work) {
    if t.danger_near(pos) {
        cost += DANGER_NEAR;
    }
    out.push(Edge { pos, spot, kind, cost, work });
}
