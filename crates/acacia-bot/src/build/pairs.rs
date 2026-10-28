//! Two-cell targets (doors, beds, tall plants; `rules::twocell`) in the survey and the plan: one placement from
//! the first cell (lower half, bed foot) fills both, so the second cell is never placed on its own; the pair is
//! done when both halves match, and a mismatch in either half breaks the first cell (BDS takes the other half
//! with it) or the junk in the second.

use std::collections::HashMap;

use acacia_physics::BlockPos;

use super::builder::{Builder, Cell};
use super::rules::twocell;
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
        (needs[i], needs[j]) = match (first, second) {
            (Need::Done, Need::Done) => (Need::Done, Need::Done),
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
