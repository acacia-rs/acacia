//! Dead ends: a node the bot cannot leave (a pit) must not end a partial path, and is not worth leaping down into.

use std::collections::VecDeque;

use acacia_physics::BlockPos;

use super::goal::Goal;
use super::moves::{self, Ctx};
use super::node::{Entry, Momentum};
use super::opts::SearchOpts;
use super::terrain::{Blocks, PosMap, Spot, Terrain};

/// Cells a classic flood from a node must reach for it to count as open; a 3×3 pit has 9.
pub(crate) const EXIT_NODES: usize = 64;

/// Classic moves (with the search's dig, bridge and door options) from `pos` reach [`EXIT_NODES`] cells.
/// `placed`: scaffold blocks the path used getting there.
pub(crate) fn has_exit<B: Blocks>(t: &Terrain<B>, pos: BlockPos, spot: Spot, opts: &SearchOpts, placed: u32) -> bool {
    flood(t, pos, spot, opts, placed, None).is_ok()
}

/// A classic flood from `pos`: `Ok` once it reaches [`EXIT_NODES`] cells (or a cell of `goal`), else every cell it
/// reached (the whole basin, none of which leads out either: whatever they reach, `pos` reaches).
fn flood<B: Blocks>(t: &Terrain<B>, pos: BlockPos, spot: Spot, opts: &SearchOpts, placed: u32, goal: Option<&Goal>) -> Result<(), Vec<BlockPos>> {
    let blocks_left = opts.kit.blocks.saturating_sub(placed);
    let ctx = Ctx { parkour: opts.parkour, dig: opts.allow_dig, bridge: opts.allow_bridge, doors: opts.open_doors, kit: &opts.kit, blocks_left, macros: None, keep: opts.keep.as_deref(), no_body: opts.no_body.as_deref(), boats: None };
    let mut seen = PosMap::default();
    seen.insert(pos, ());
    let (mut queue, mut edges, rest) = (VecDeque::from([(pos, spot)]), Vec::new(), Momentum::default());
    if goal.is_some_and(|g| g.success(pos)) {
        return Ok(());
    }
    while let Some((p, s)) = queue.pop_front() {
        edges.clear();
        moves::successors(t, p, s, (Entry::Rest, &rest), &ctx, &mut edges);
        for e in &edges {
            if seen.insert(e.pos, ()).is_none() {
                if seen.len() >= EXIT_NODES || goal.is_some_and(|g| g.success(e.pos)) {
                    return Ok(());
                }
                queue.push_back((e.pos, e.spot));
            }
        }
    }
    Err(seen.into_keys().collect())
}

/// Leap landings checked for a way on (classic moves to [`EXIT_NODES`] cells or the goal), each cell once a search.
/// Heuristic like [`has_exit`]: a basin left only by a leap the classic gap jump cannot make counts as closed.
#[derive(Default)]
pub(crate) struct Exits(PosMap<bool>);

impl Exits {
    pub fn open<B: Blocks>(&mut self, t: &Terrain<B>, pos: BlockPos, spot: Spot, opts: &SearchOpts, goal: &Goal) -> bool {
        if let Some(&open) = self.0.get(&pos) {
            return open;
        }
        match flood(t, pos, spot, opts, 0, Some(goal)) {
            Ok(()) => {
                self.0.insert(pos, true);
                true
            }
            Err(basin) => {
                basin.into_iter().for_each(|c| _ = self.0.insert(c, false));
                false
            }
        }
    }
}
