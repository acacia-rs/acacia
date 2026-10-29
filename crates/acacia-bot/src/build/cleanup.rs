//! Removing scaffold once everything is placed: the nearest block with no scaffold on it, so a pillar comes down
//! top down and then the next one. Swept by layer instead (every top, then the layer under), the house16 sim's one
//! bot crossed the house five times: 27 walks and ~1000 of 3400 ticks.

use acacia_physics::BlockPos;

use super::builder::Builder;
use super::plan::{break_at, dist, Next, TRIES};
use super::status::{HoldWhy, Phase};
use crate::pathfind::process::Ctx;

/// Breaks the next scaffold block or goes where it can; holds while what stands cannot be broken now; `None` once
/// none stands. Seals outside the footprint stay.
pub(super) fn next(b: &mut Builder, cx: &Ctx, committed: Option<BlockPos>) -> Option<Next> {
    let standing: Vec<BlockPos> = b.scaffold.iter().copied().filter(|p| !b.seals.contains(p) && !b.given_up(*p) && cx.look.solid(*p) == Some(true)).filter(|p| b.target(*p).is_none_or(|t| t.is_air())).collect();
    let covered = |p: &BlockPos| standing.contains(&[p[0], p[1] + 1, p[2]]);
    let eye = cx.eye();
    let mut left: Vec<BlockPos> = standing.iter().copied().filter(|p| !b.resting(*p) && b.mine(*p)).collect();
    left.sort_by(|p, q| (Some(*q) == committed).cmp(&(Some(*p) == committed)).then(covered(p).cmp(&covered(q))).then(dist(eye, *p).total_cmp(&dist(eye, *q))));
    for &pos in left.iter().take(TRIES) {
        match break_at(cx, pos, Phase::Cleaning, b) {
            Some(next) => return Some(next),
            None => b.rest(pos),
        }
    }
    (!standing.is_empty()).then(|| Next::Hold(Phase::Cleaning, Vec::new(), HoldWhy::Cleaning))
}
