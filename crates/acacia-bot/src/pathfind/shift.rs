//! Moving a cached macro-move result to another node (`cache.rs`): positions are translated in f32, so a reused
//! landing may sit a float rounding off the one a fresh rollout would give, far under `Momentum`'s quantum.

use acacia_physics::BlockPos;

use super::control::RunCtl;
use super::hop::Ran;
use super::leap::LeapCtl;
use super::solve::{Miss, Solved};
use super::terrain::Spot;
use crate::movement::Motion;

/// Moved to another node `d` away.
pub(crate) trait Shift {
    fn shift(&mut self, d: BlockPos);
}

pub(crate) fn add(p: BlockPos, d: BlockPos) -> BlockPos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

impl Shift for BlockPos {
    fn shift(&mut self, d: BlockPos) {
        *self = add(*self, d);
    }
}

impl Shift for Spot {
    fn shift(&mut self, d: BlockPos) {
        self.feet += d[1] as f32;
    }
}

/// Only the position: a stored end motion is read for its position, velocity and flags.
impl Shift for Motion {
    fn shift(&mut self, d: BlockPos) {
        (0..3).for_each(|i| self.pos[i] += d[i] as f32);
    }
}

impl Shift for LeapCtl {
    fn shift(&mut self, d: BlockPos) {
        self.target.shift(d);
        self.feet += d[1] as f32;
    }
}

impl Shift for RunCtl {
    fn shift(&mut self, d: BlockPos) {
        self.target.shift(d);
        self.feet += d[1] as f32;
    }
}

impl Shift for Solved {
    fn shift(&mut self, d: BlockPos) {
        self.ctl.shift(d);
        self.end.shift(d);
    }
}

impl Shift for Ran {
    fn shift(&mut self, d: BlockPos) {
        self.ctl.shift(d);
        self.end.shift(d);
    }
}

impl<T: Shift> Shift for Option<T> {
    fn shift(&mut self, d: BlockPos) {
        if let Some(v) = self {
            v.shift(d);
        }
    }
}

impl<T: Shift> Shift for Result<T, Miss> {
    fn shift(&mut self, d: BlockPos) {
        if let Ok(v) = self {
            v.shift(d);
        }
    }
}

impl<A: Shift, B: Shift, C: Shift> Shift for (A, B, C) {
    fn shift(&mut self, d: BlockPos) {
        self.0.shift(d);
        self.1.shift(d);
        self.2.shift(d);
    }
}
