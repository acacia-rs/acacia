//! Scaffold that gives a placement something to click: a column up to a cell with no solid neighbour, and a prop
//! on the axis of a log whose only neighbours face the wrong way.

use acacia_physics::{BlockPos, Vec3};

use super::builder::Builder;
use super::plan::{fill, free, go, no_stand, unbuilt, Next, STANDS};
use super::reach::{fills_from, stands, Keep};
use super::status::Phase;
use super::structure::Block;
use crate::pathfind::process::{Ctx, Look};

/// Fills the lowest empty cell of the column under `pos` (it stands on something), so the column grows up to it.
pub(super) fn column(b: &Builder, cx: &Ctx, pos: BlockPos) -> Option<Next> {
    let mut cell = [pos[0], pos[1] - 1, pos[2]];
    while cx.look.solid([cell[0], cell[1] - 1, cell[2]]) == Some(false) {
        cell[1] -= 1;
        if pos[1] - cell[1] > 16 {
            return None;
        }
    }
    if b.target(cell).is_some_and(|t| !t.is_air()) {
        return None;
    }
    let ok = |e: Vec3| fills_from(e, cell, cx.look);
    if ok(cx.eye()) && free(b, cx, cell) {
        return Some(Next::Act(Phase::Building, fill(b, cell)));
    }
    go(Phase::Building, cell, stands(cell, cx.pos, cx.look, &Keep { feet: &|f| f == cell || no_stand(b, cx.look, f), pillar: &|c| unbuilt(b, cx.look, c) }, &ok, STANDS), b)
}

/// The cell to prop a log from: its axis takes a click on a face along it, so with neither cell along the axis
/// solid, the first one the structure leaves empty (live: a taiga house's eave logs run along z beside
/// cobblestone, and 72 rested for want of a stand). Only once the cell has rested: a neighbour may come first.
pub(super) fn prop_for(b: &Builder, look: &dyn Look, pos: BlockPos, target: &Block) -> Option<BlockPos> {
    let d = match target.prop("pillar_axis")? {
        "x" => [1, 0, 0],
        "y" => [0, 1, 0],
        "z" => [0, 0, 1],
        _ => return None,
    };
    let along = [1, -1].map(|s| [pos[0] + s * d[0], pos[1] + s * d[1], pos[2] + s * d[2]]);
    if !b.rested(pos) || along.iter().any(|c| look.solid(*c) != Some(false)) {
        return None;
    }
    along.into_iter().find(|c| b.target(*c).is_none_or(Block::is_air))
}

/// Builds the prop at `cell`: the column under the cell above it, ending in the cell.
pub(super) fn prop(b: &Builder, cx: &Ctx, cell: BlockPos) -> Option<Next> {
    column(b, cx, [cell[0], cell[1] + 1, cell[2]])
}
