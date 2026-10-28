//! Two-cell targets (doors, beds, tall plants; `rules::twocell`) in the survey and the plan: one placement from
//! the first cell (lower half, bed foot) fills both, so the second cell is never placed on its own; the pair is
//! done when both halves match, and a mismatch in either half breaks the first cell (BDS takes the other half
//! with it) or the junk in the second.

use std::collections::{HashMap, HashSet};

use acacia_physics::BlockPos;

use super::builder::{Builder, Cell};
use super::rules::{self, twocell, twocell::Kind};
use super::structure::Block;
use super::survey::Need;
use crate::pathfind::process::Look;

/// Rewrites the per-cell needs of every pair in the footprint.
pub(super) fn settle(cells: &[Cell], index: &HashMap<BlockPos, usize>, look: &dyn Look, fill_only: bool, needs: &mut [Need]) {
    let wrong = if fill_only { Need::Kept } else { Need::Break };
    for (i, c) in cells.iter().enumerate() {
        let Some((other, first)) = twocell::partner(&c.target, c.pos) else { continue };
        // Half a pair in the footprint (BDS would place the other half outside it), or a malformed one.
        let Some(&j) = index.get(&other).filter(|&&j| cells[j].target.name == c.target.name) else {
            needs[i] = Need::Unsupported;
            continue;
        };
        if !first {
            continue;
        }
        let (first, second) = (needs[i], needs[j]);
        // The pair shares what one half can't be: never placed with half of it unknown or unsupported.
        if let Some(n) = [Need::Unsupported, Need::Unknown].into_iter().find(|n| *n == first || *n == second) {
            (needs[i], needs[j]) = (n, n);
            continue;
        }
        let occupied = |p: BlockPos| look.block(p).is_some_and(|v| v.solid());
        // A bed's colour from its block entity; none known yet (just placed, or not sent) is no mismatch.
        let colored = || twocell::color(&c.target).is_none_or(|want| [c.pos, other].iter().find_map(|p| look.color(*p)).is_none_or(|have| have == want));
        (needs[i], needs[j]) = match (first, second) {
            (Need::Done, Need::Done) if colored() => (Need::Done, Need::Done),
            (Need::Place, _) if occupied(other) => (if fill_only { Need::Kept } else { Need::Paired }, wrong),
            (Need::Place, _) => (Need::Place, Need::Paired),
            // Either half wrong with the first cell filled: break the first; what is left in the second goes next.
            _ => (wrong, Need::Paired),
        };
    }
}

/// The second cell of the pair placed from `pos`: it must be free of bodies too.
pub(super) fn second(b: &Builder, pos: BlockPos) -> Option<BlockPos> {
    b.target(pos).and_then(|t| twocell::partner(t, pos)).filter(|(_, first)| *first).map(|(other, _)| other)
}

/// The first cell of a pair can't be placed yet: the server would give a door another hinge from the
/// neighbours as they stand now (`twocell::hinge_now`).
pub(super) fn waits(b: &Builder, look: &dyn Look, pos: BlockPos) -> bool {
    let Some(other) = second(b, pos) else { return false };
    b.target(other).is_some_and(|upper| twocell::hinge_now(upper, other, look).is_some_and(|h| Some(h) != upper.prop("door_hinge_bit")))
}

/// Rests (40 ticks each) a held cell, or the door holding it, may wait before the hold is let go: a door no bot
/// places (no stand, another bot's band gone, its hinge never right) must not keep its neighbours unbuilt.
const HOLD_RESTS: u32 = 20;

/// Cells held back this tick: neighbours of a door not yet in that, built before it, would give it another hinge
/// than its target (with every neighbour built, the door's hinge would come out wrong). They go in after it; the
/// door itself waits for the neighbours it does need (`waits`).
pub(super) fn held(b: &Builder, look: &dyn Look, needs: &[Need]) -> HashSet<BlockPos> {
    let patient = |p: BlockPos| !b.given_up(p) && b.tries.get(&p).is_none_or(|t| t.rests < HOLD_RESTS);
    let mut out = HashSet::new();
    for (i, c) in b.cells.iter().enumerate() {
        let open = matches!(needs[i], Need::Place | Need::Paired | Need::Break) && patient(c.pos);
        let Some(upper_pos) = second(b, c.pos).filter(|_| open && twocell::kind(&c.target.name) == Some(Kind::Door)) else { continue };
        if let Some(upper) = b.target(upper_pos) {
            out.extend(after(b, look, upper, upper_pos).into_iter().filter(|p| patient(*p)));
        }
    }
    out
}

/// The unbuilt neighbour targets of the door whose upper half `upper` is at `pos` that must wait for it: the
/// fewest that leave the hinge right with the rest built first. Empty when every order works or none does.
fn after(b: &Builder, look: &dyn Look, upper: &Block, pos: BlockPos) -> Vec<BlockPos> {
    let Some((l, r)) = twocell::sides(upper, pos) else { return Vec::new() };
    let want = upper.prop("door_hinge_bit");
    let near = [l, [l[0], l[1] + 1, l[2]], r, [r[0], r[1] + 1, r[2]]];
    let built = |p: BlockPos| b.target(p).is_some_and(|t| look.block(p).is_some_and(|v| rules::matches(t, &v)));
    let open: Vec<BlockPos> = near.into_iter().filter(|p| b.target(*p).is_some_and(|t| !t.is_air()) && !built(*p)).collect();
    // Subsets of `open` by bit, most cells built first.
    let mut masks: Vec<u32> = (0..1u32 << open.len()).collect();
    masks.sort_by_key(|m| std::cmp::Reverse(m.count_ones()));
    let at = |mask: u32, p: BlockPos| match (open.iter().position(|o| *o == p), b.target(p)) {
        (Some(k), _) if mask & (1 << k) == 0 => None,
        (_, Some(t)) if t.is_air() => None,
        (_, Some(t)) => Some((t.name.clone(), t.props.clone())),
        (_, None) => look.block(p).map(|v| (v.name.to_string(), v.props.to_string())),
    };
    match masks.iter().find(|&&m| twocell::hinge_with(upper, pos, &|p| at(m, p)) == want) {
        Some(&m) => open.iter().enumerate().filter(|(k, _)| m & (1 << k) == 0).map(|(_, p)| *p).collect(),
        None => Vec::new(),
    }
}
