//! Blocks that wall the bot in where it stands, broken to get it out (the survey rebuilds them).

use acacia_physics::BlockPos;

use super::builder::Builder;
use super::rules;
use crate::pathfind::process::Ctx;

/// Failed walks in a row after which path work may break and fill built cells (`process::opts`).
pub(super) const ESCAPE_AFTER: u32 = 2;

impl Builder {
    /// After failed walks, a thin block (pane, fence, wall, bars) in the feet or head cell: the body fits beside
    /// it but the pathfinder sees the cell as solid, so no walk starts (live and sim: the outer half of a window
    /// cell, every walk "Stuck 10 times" until the run ended).
    /// A door in a cell the body overlaps once a walk failed: broken. Once inside, every tick was a movement
    /// correction and every walk failed (live 2026-10-28, five vanilla houses); walks through open doors that stop
    /// to act on the way are not wedged (sim: a door broken on every pass).
    pub(super) fn wedged(&self, cx: &Ctx) -> Option<BlockPos> {
        if !cx.on_ground {
            return None;
        }
        let door = |c: &BlockPos| cx.look.block(*c).is_some_and(|v| rules::twocell::kind(v.name) == Some(rules::twocell::Kind::Door));
        let y = cx.pos[1].floor() as i32;
        // ±0.2, not the body's 0.3: a stand reached 0.25 off-centre puts the body 0.05 into a neighbouring door's cell.
        let body = [-0.2, 0.2].into_iter().flat_map(|dx| [-0.2, 0.2].map(|dz| [(cx.pos[0] + dx).floor() as i32, (cx.pos[2] + dz).floor() as i32]));
        if self.failed_walks > 0
            && let Some(c) = body.flat_map(|[x, z]| [[x, y, z], [x, y + 1, z]]).find(door)
        {
            return Some(c);
        }
        if self.failed_walks < ESCAPE_AFTER {
            return None;
        }
        let feet = cx.pos.map(|v| v.floor() as i32);
        [feet, [feet[0], feet[1] + 1, feet[2]]].into_iter().find(|c| cx.look.block(*c).is_some_and(|v| v.solid() && !v.full && rules::thin(v.name)))
    }
}
