//! What a move changes in the world before the bot can make it.

use acacia_physics::BlockPos;

use crate::interact::Face;

/// One world change, done standing at the move's start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Break(BlockPos),
    /// Place a scaffold block against `face` of `against`.
    Place { against: BlockPos, face: Face },
    /// Open (or close) a door, fence gate or trapdoor so it no longer blocks the move.
    Door { pos: BlockPos, open: bool },
}

const MAX_STEPS: usize = 6;

/// The steps of one move, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Work {
    steps: [Option<Step>; MAX_STEPS],
    len: u8,
}

impl Work {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn steps(&self) -> impl Iterator<Item = Step> + '_ {
        self.steps[..usize::from(self.len)].iter().flatten().copied()
    }

    /// Adds `step` unless already present; false if it was (or there is no room).
    pub(crate) fn push(&mut self, step: Step) -> bool {
        if self.steps().any(|s| s == step) || usize::from(self.len) == MAX_STEPS {
            return false;
        }
        self.steps[usize::from(self.len)] = Some(step);
        self.len += 1;
        true
    }

    pub fn breaks(&self) -> usize {
        self.steps().filter(|s| matches!(s, Step::Break(_))).count()
    }

    pub fn places(&self) -> u32 {
        self.steps().filter(|s| matches!(s, Step::Place { .. })).count() as u32
    }
}

/// The face of a block that looks towards horizontal direction `(dx, dz)`.
pub(crate) fn face_toward(dx: i32, dz: i32) -> Face {
    match (dx, dz) {
        (1, _) => Face::East,
        (-1, _) => Face::West,
        (_, 1) => Face::South,
        _ => Face::North,
    }
}
