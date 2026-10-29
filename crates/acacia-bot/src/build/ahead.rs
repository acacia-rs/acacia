//! Placing far to near, bottom up. A click needs a face the eye sees, so a row above the eye or a floor around the
//! bot is placed from one spot only far end first: each new cell is then clicked on its far neighbour, already
//! there, with the near cells still open for the ray. A column grows one cell a tick (a cell never covers an open
//! one), so a stand is scored on the whole column over a ready cell, not just its open bottom. Stands are scored
//! on that order ([`reaches`]) and the bot places in it ([`far_first`]).

use std::collections::HashMap;

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockRegistry;

use super::builder::Builder;
use super::plan::dist;
use super::reach::click_for;
use super::structure::Block;
use crate::pathfind::process::{BlockView, Look, Target};

/// A cell to place, as its target will show once placed.
pub(super) struct Cell<'a> {
    pos: BlockPos,
    target: &'a Block,
    view: Option<BlockView>,
}

fn view(t: &Block) -> Option<BlockView> {
    let r = BlockRegistry::vanilla();
    r.find(&t.name, &t.props).or_else(|| r.find(&t.name, "")).and_then(|id| r.get(id)).map(BlockView::of)
}

/// The `ready` cells and, over each, the open cells of the same block stacked on it.
pub(super) fn cells<'a>(b: &'a Builder, look: &dyn Look, ready: &[(usize, String)]) -> Vec<Cell<'a>> {
    let mut out = Vec::new();
    for (i, _) in ready {
        let mut pos = b.cells[*i].pos;
        let name = &b.cells[*i].target.name;
        while let Some(target) = b.target(pos).filter(|t| &t.name == name && (pos == b.cells[*i].pos || look.solid(pos) == Some(false))) {
            out.push(Cell { pos, target, view: view(target) });
            pos[1] += 1;
        }
    }
    out
}

/// The world with `placed` already built.
struct Ahead<'a> {
    base: &'a dyn Look,
    placed: &'a HashMap<BlockPos, BlockView>,
}

impl Look for Ahead<'_> {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        self.placed.get(&p).map(|v| v.solid()).or_else(|| self.base.solid(p))
    }

    fn find(&self, target: &Target) -> Option<Vec3> {
        self.base.find(target)
    }

    fn block(&self, p: BlockPos) -> Option<BlockView> {
        self.placed.get(&p).copied().or_else(|| self.base.block(p))
    }

    fn held(&self) -> Option<&str> {
        self.base.held()
    }
}

/// The `cells` placed from `eye` bottom up and far to near, each against what is built or placed before it; a
/// column stops at its first cell out of reach.
pub(super) fn reaches(cells: &[Cell], eye: Vec3, look: &dyn Look, slack: f32) -> Vec<BlockPos> {
    let mut order: Vec<&Cell> = cells.iter().collect();
    order.sort_by(|a, b| a.pos[1].cmp(&b.pos[1]).then(dist(eye, b.pos).total_cmp(&dist(eye, a.pos))));
    let mut placed = HashMap::new();
    let mut out = Vec::new();
    for c in order {
        let below = [c.pos[0], c.pos[1] - 1, c.pos[2]];
        let stacked = cells.iter().any(|o| o.pos == below);
        if (!stacked || out.contains(&below)) && click_for(c.pos, c.target, eye, &Ahead { base: look, placed: &placed }, slack).is_some() {
            placed.extend(c.view.map(|v| (c.pos, v)));
            out.push(c.pos);
        }
    }
    out
}

/// `pos` waits for an open cell beside it at its height that only a top-face click gives
/// ([`super::rules::only_from_above`]), unless it is one too: placed first, it hides the face below that cell from
/// the stands that build it (house16: walls round a corner left its upright logs for walks at the end).
pub(super) fn waits_for_upright(b: &Builder, open: impl Fn(usize) -> bool, pos: BlockPos) -> bool {
    let upright = |j: usize| super::rules::only_from_above(&b.cells[j].target);
    let beside = |[dx, dz]: [i32; 2]| b.index.get(&[pos[0] + dx, pos[1], pos[2] + dz]).copied();
    !b.index.get(&pos).is_some_and(|&i| upright(i)) && [[1, 0], [-1, 0], [0, 1], [0, -1]].into_iter().filter_map(beside).any(|j| open(j) && upright(j))
}

/// `ready` with its first rank (held item, lowest layer) farthest first, the rest as it was.
pub(super) fn far_first<'a>(ready: &'a [(usize, String)], eye: Vec3, pos: impl Fn(usize) -> BlockPos, held: Option<&str>) -> Vec<&'a (usize, String)> {
    let rank = |(i, item): &(usize, String)| (Some(item.as_str()) != held, pos(*i)[1]);
    let lead = ready.iter().take_while(|r| Some(rank(r)) == ready.first().map(rank)).count();
    let mut out: Vec<&(usize, String)> = ready.iter().collect();
    out[..lead].sort_by(|(i, _), (j, _)| dist(eye, pos(*j)).total_cmp(&dist(eye, pos(*i))));
    out
}
