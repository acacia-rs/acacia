//! Moves that change the world first (after Baritone's breaking traverse, ascend and downward
//! moves, bridging and pillaring): see [`Work`]. Each starts standing dry on a block top and is
//! offered only where the plain moves found no way.

use acacia_physics::BlockPos;

use super::cell::{Cell, AVOID, FALLS, LIQUID, OPENABLE, Rect, UNLOADED};
use super::costs::{self, BREAK_PENALTY, CENTER_AFTER_FALL, DOOR, MAX_DIG_TICKS, SNEAK, WALK};
use super::kit::Kit;
use super::moves::{offset, push, Ctx, Edge, MoveKind};
use super::terrain::{Blocks, Span, Spot, SpotKind, Terrain, CARVED, EPS, OPENED, PLACED_FLOOR};
use super::work::{face_toward, Step, Work};
use crate::interact::Face;

/// Where liquid could flow into a broken cell from: the sides and above.
const FLOW_IN: [(i32, i32, i32); 5] = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0)];

fn grounded(p: BlockPos, from: Spot) -> bool {
    from.kind == SpotKind::Stand && !from.wet && (from.feet - p[1] as f32).abs() < EPS
}

/// The block under `p` takes a placed block on its faces.
fn solid_under<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot) -> bool {
    from.made & PLACED_FLOOR != 0 || t.cell(offset(p, 0, -1, 0)).is_floor()
}

/// Ticks to break the block at `q` (plus [`BREAK_PENALTY`]), or `None` if it must stay: unbreakable,
/// too slow, liquid in or next to it, or a falling block resting on it.
pub(crate) fn dig_cost<B: Blocks>(t: &Terrain<B>, q: BlockPos, kit: &Kit) -> Option<f32> {
    if t.cell(q).has(UNLOADED | AVOID | LIQUID) {
        return None;
    }
    let ticks = kit.break_ticks(&t.state(q)?.mining).filter(|&n| n <= MAX_DIG_TICKS)?;
    if t.cell(offset(q, 0, 1, 0)).has(FALLS) {
        return None;
    }
    let flood = FLOW_IN.iter().any(|&(dx, dy, dz)| t.cell(offset(q, dx, dy, dz)).has(LIQUID | UNLOADED));
    (!flood).then_some(ticks as f32 + BREAK_PENALTY)
}

/// Work that clears the cells in a move's way, and what it costs.
struct Clearing<'c, 'k> {
    ctx: &'c Ctx<'k>,
    dig: bool,
    work: Work,
    cost: f32,
}

impl<'c, 'k> Clearing<'c, 'k> {
    fn new(ctx: &'c Ctx<'k>) -> Self {
        Self { ctx, dig: ctx.dig, work: Work::default(), cost: 0.0 }
    }

    /// Opens the cell if toggling it frees `span`, else breaks it if digging is allowed.
    fn clear<B: Blocks>(&mut self, t: &Terrain<B>, q: BlockPos, c: Cell, span: Span) -> bool {
        if self.ctx.doors && c.has(OPENABLE) && t.toggled_boxes(c.block).is_some_and(|b| !span.hits(b)) {
            let open = t.state(q).and_then(|s| s.property("open_bit")) != Some("1");
            if self.work.push(Step::Door { pos: door_base(t, q), open }) {
                self.cost += DOOR;
            }
            return true;
        }
        let Some(cost) = self.dig.then(|| dig_cost(t, q, self.ctx.kit)).flatten() else { return false };
        if self.work.push(Step::Break(q)) {
            self.cost += cost;
        }
        true
    }
}

/// A door's lower half (both halves toggle together), else `q`.
fn door_base<B: Blocks>(t: &Terrain<B>, q: BlockPos) -> BlockPos {
    let below = offset(q, 0, -1, 0);
    match (t.state(q), t.state(below)) {
        (Some(s), Some(b)) if s.property("upper_block_bit") == Some("1") && b.name == s.name => below,
        _ => q,
    }
}

/// Cardinal moves at the same level or one up that open, dig or bridge, where no plain move reached.
pub(crate) fn forward<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, (dx, dz): (i32, i32), ctx: &Ctx, start: usize, out: &mut Vec<Edge>) {
    if !grounded(p, from) {
        return;
    }
    let side = offset(p, dx, 0, dz);
    let up = offset(side, 0, 1, 0);
    let (to_side, to_up) = (out[start..].iter().any(|e| e.pos == side), out[start..].iter().any(|e| e.pos == up));
    if !to_side {
        level(t, p, from, (dx, dz), ctx, out);
    }
    if ctx.dig && !to_up {
        ascend(t, p, from, (dx, dz), ctx, out);
    }
}

/// Into the cell ahead at feet level: open or dig through what is in the way, and place a floor
/// first when there is none.
fn level<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, (dx, dz): (i32, i32), ctx: &Ctx, out: &mut Vec<Edge>) {
    let side = offset(p, dx, 0, dz);
    let below = t.cell(offset(side, 0, -1, 0));
    let bridge = !below.is_floor();
    if bridge && !(ctx.bridge && ctx.blocks_left > 0 && below.is_open() && solid_under(t, p, from)) {
        return;
    }
    let feet = p[1] as f32;
    let mut cl = Clearing::new(ctx);
    let ok = t.obstruct_from(p, from, Rect::toward(dx, dz), feet, |q, c, s| cl.clear(t, q, c, s))
        && t.obstruct(side, Rect::toward(-dx, -dz), feet, |q, c, s| cl.clear(t, q, c, s));
    if !ok {
        return;
    }
    let mut made = if cl.work.breaks() > 0 { CARVED } else { 0 };
    if cl.work.steps().any(|s| matches!(s, Step::Door { pos, .. } if pos[0] == side[0] && pos[2] == side[2])) {
        made |= OPENED;
    }
    let mut cost = WALK + cl.cost;
    if bridge {
        cl.work.push(Step::Place { against: offset(p, 0, -1, 0), face: face_toward(dx, dz) });
        made |= PLACED_FLOOR;
        cost += SNEAK - WALK + costs::place(ctx.blocks_left);
    }
    if !cl.work.is_empty() {
        push(t, out, side, Spot::stand(feet, made), MoveKind::Walk, cost, cl.work);
    }
}

/// Jump onto the block ahead after digging out the head room on both sides.
fn ascend<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, (dx, dz): (i32, i32), ctx: &Ctx, out: &mut Vec<Edge>) {
    let up = offset(p, dx, 1, dz);
    if !t.cell(offset(up, 0, -1, 0)).is_floor() {
        return;
    }
    let feet = p[1] as f32 + 1.0;
    let mut cl = Clearing::new(ctx);
    let ok = t.obstruct_from(p, from, Rect::FOOT, feet, |q, c, s| cl.clear(t, q, c, s))
        && t.obstruct_from(p, from, Rect::toward(dx, dz), feet, |q, c, s| cl.clear(t, q, c, s))
        && t.obstruct(up, Rect::toward(-dx, -dz), feet, |q, c, s| cl.clear(t, q, c, s));
    if ok && cl.work.breaks() > 0 {
        push(t, out, up, Spot::stand(feet, CARVED), MoveKind::Ascend, costs::ascend() + cl.cost, cl.work);
    }
}

/// Digging down and pillaring up.
pub(crate) fn vertical<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, ctx: &Ctx, out: &mut Vec<Edge>) {
    if !grounded(p, from) {
        return;
    }
    if ctx.dig {
        down(t, p, from, ctx, out);
    }
    if ctx.bridge && ctx.blocks_left > 0 {
        pillar(t, p, from, ctx, out);
    }
}

fn down<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, ctx: &Ctx, out: &mut Vec<Edge>) {
    let below = offset(p, 0, -1, 0);
    if from.made & PLACED_FLOOR != 0 || !t.cell(offset(p, 0, -2, 0)).is_floor() {
        return;
    }
    let Some(cost) = dig_cost(t, below, ctx.kit) else { return };
    let mut work = Work::default();
    work.push(Step::Break(below));
    let cost = cost + costs::fall_ticks(1.0) + CENTER_AFTER_FALL;
    push(t, out, below, Spot::stand(below[1] as f32, CARVED), MoveKind::Down, cost, work);
}

fn pillar<B: Blocks>(t: &Terrain<B>, p: BlockPos, from: Spot, ctx: &Ctx, out: &mut Vec<Edge>) {
    let carved = from.made & CARVED != 0;
    if !solid_under(t, p, from) || !(carved || t.cell(p).is_open()) {
        return;
    }
    let feet = p[1] as f32 + 1.0;
    let mut cl = Clearing::new(ctx);
    if !t.obstruct_from(p, from, Rect::FOOT, feet, |q, c, s| !c.has(OPENABLE) && cl.clear(t, q, c, s)) {
        return;
    }
    let made = PLACED_FLOOR | if carved || cl.work.breaks() > 0 { CARVED } else { 0 };
    cl.work.push(Step::Place { against: offset(p, 0, -1, 0), face: Face::Up });
    let cost = costs::ascend() + costs::place(ctx.blocks_left) + cl.cost;
    push(t, out, offset(p, 0, 1, 0), Spot::stand(feet, made), MoveKind::Pillar, cost, cl.work);
}
