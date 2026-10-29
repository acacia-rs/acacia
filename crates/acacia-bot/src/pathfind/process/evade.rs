//! Runs from a lit creeper (design decision 3): above the other processes, it takes control while one is lit within
//! [`CLEAR`] and drives [`Goal::Away`] from where it stood; the process it pre-empted re-plans when it hands back.

use acacia_physics::{BlockPos, Vec3};

use super::{Ctx, Process, Want};
use crate::pathfind::{Goal, GotoOpts};

/// Blocks from a lit creeper that are safe: past its blast (6) and where it stops its fuse (7).
pub const CLEAR: f32 = 8.0;
/// A creeper that moved this far from where the run started is run from anew.
const RESTART: f32 = 2.0;

pub struct Evade {
    opts: GotoOpts,
    from: Option<Vec3>,
}

impl Evade {
    /// `opts`: the bot's driving settings (its profile above all).
    pub fn new(opts: GotoOpts) -> Self {
        Self { opts, from: None }
    }
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn cell(p: Vec3) -> BlockPos {
    p.map(|v| v.floor() as i32)
}

impl Process for Evade {
    fn name(&self) -> &str {
        "evade"
    }

    fn want(&mut self, cx: &Ctx) -> Want {
        let blasts = cx.hazards.map_or(&[][..], |h| &h.blasts[..]);
        let Some(&near) = blasts.iter().filter(|&&b| dist(cx.pos, b) < CLEAR).min_by(|a, b| dist(cx.pos, **a).total_cmp(&dist(cx.pos, **b))) else {
            self.from = None;
            return Want::Yield;
        };
        let from = *self.from.get_or_insert(near);
        let from = if dist(from, near) > RESTART { *self.from.insert(near) } else { from };
        Want::Go(Goal::Away(cell(from), CLEAR))
    }

    fn opts(&self) -> GotoOpts {
        self.opts.clone()
    }
}
