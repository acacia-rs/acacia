//! What the pilot watches besides the path: server setbacks (hold still, then re-plan from where the server
//! put the bot) and work issued again at the same cell (a loop: the server keeps undoing it).
//! Design: acacia-pathfind/docs/confirmed-actions.md.

use std::collections::HashMap;

use super::Pilot;
use crate::governor::SETTLE_TICKS;
use crate::movement::{Live, Sim};
use crate::pathfind::{PathNode, Step};
use crate::ActionError;

impl Pilot {
    /// Server setbacks the pilot held still for.
    pub fn settles(&self) -> u32 {
        self.watch.settles
    }

    /// Path work steps issued again at a cell they were issued at before.
    pub fn work_repeats(&self) -> u32 {
        self.watch.repeats
    }

    /// Within [`SETTLE_TICKS`] of a setback: stops the controls (dropping the path on a new one); true while holding.
    pub(super) fn hold_after_setback(&mut self, movement: &mut impl Live) -> bool {
        match self.watch.setback(movement.setback_age()) {
            Hold::No => return false,
            Hold::Started => {
                tracing::debug!(pos = ?movement.base().map(|s| s.body().pos), "server setback; holding before re-planning");
                self.interrupt();
            }
            Hold::Holding => {}
        }
        movement.controls().stop();
        true
    }
}

/// A step issued this many times is a loop.
const MAX_SAME_WORK: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hold {
    No,
    /// A new setback: drop the path.
    Started,
    Holding,
}

#[derive(Default)]
pub(super) struct Watch {
    holding: bool,
    pub(super) settles: u32,
    issued: HashMap<Step, u32>,
    pub(super) repeats: u32,
}

impl Watch {
    /// `setback_age`: ticks since the movement's last setback.
    pub(super) fn setback(&mut self, setback_age: Option<u64>) -> Hold {
        let recent = setback_age.is_some_and(|a| a < SETTLE_TICKS);
        let was = std::mem::replace(&mut self.holding, recent);
        match (recent, was) {
            (false, _) => Hold::No,
            (true, false) => {
                self.settles += 1;
                Hold::Started
            }
            (true, true) => Hold::Holding,
        }
    }

    /// `node`'s work is about to be done; an error once one of its steps has been issued [`MAX_SAME_WORK`] times.
    pub(super) fn work(&mut self, node: &PathNode) -> Result<(), ActionError> {
        for step in node.work.steps() {
            let n = self.issued.entry(step).or_default();
            *n += 1;
            if *n > 1 {
                self.repeats += 1;
                tracing::debug!(?step, times = *n, "path work issued again");
            }
            if *n >= MAX_SAME_WORK {
                return Err(ActionError::NotPossible(format!("loop: {step:?} issued {n} times")));
            }
        }
        Ok(())
    }
}
