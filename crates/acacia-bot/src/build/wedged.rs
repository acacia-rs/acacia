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
    /// Doors are not broken here: no stand is in a door's cell (`plan::no_stand`), and breaking one round the body
    /// timed out every time (live 2026-10-29: 59 to 63 ten-second timeouts a house, runs cut by the watchdog).
    pub(super) fn wedged(&self, cx: &Ctx) -> Option<BlockPos> {
        if !cx.on_ground || self.failed_walks < ESCAPE_AFTER {
            return None;
        }
        let feet = cx.pos.map(|v| v.floor() as i32);
        [feet, [feet[0], feet[1] + 1, feet[2]]].into_iter().find(|c| cx.look.block(*c).is_some_and(|v| v.solid() && !v.full && rules::thin(v.name)))
    }
}
