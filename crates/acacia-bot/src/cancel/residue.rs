//! What actions hold on the bot while they run (see [`crate::cancel`]). An action takes a [`Claim`] and gives it
//! back when it finishes; the claim lives in the action's future, so a dropped future leaves its entry behind with
//! no owner, and the bot's next wait releases it (`Drop` cannot reach the bot).

use std::sync::{Arc, Weak};

use acacia_physics::BlockPos;

use crate::Bot;

/// Something an action took, and what releasing it undoes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Held {
    /// A governor admission: settled.
    Slot,
    /// A block being mined: `AbortBreak`, or BDS keeps destroying the cell.
    Break(BlockPos),
    /// The own inventory screen, opened for stack requests: closed.
    InventoryScreen,
    /// Jump pressed for a tick: back to this.
    Jump(bool),
    /// A processes run (goto): its movement keys let go, its pipelined placements forgotten.
    Processes,
    /// A dismount not done yet: called off.
    Dismount,
    /// A bed clicked, `StartSleeping` not sent yet: left if the server put the player in it.
    Bed,
}

/// An action's hold on one [`Held`]: give it back with [`Bot::release`] or [`Bot::unclaim`].
#[must_use = "a dropped claim is released at the next wait"]
#[derive(Debug)]
pub(crate) struct Claim(Arc<()>);

#[derive(Debug, Default)]
pub(crate) struct Residue {
    held: Vec<(Weak<()>, Held)>,
}

impl Residue {
    fn take(&mut self, claim: &Claim) -> Option<Held> {
        let i = self.held.iter().position(|(owner, _)| std::ptr::eq(owner.as_ptr(), Arc::as_ptr(&claim.0)))?;
        Some(self.held.remove(i).1)
    }

    /// Entries whose claim was dropped, newest first.
    fn orphans(&mut self) -> Vec<Held> {
        let mut orphans: Vec<Held> = self.held.extract_if(.., |(owner, _)| owner.strong_count() == 0).map(|(_, held)| held).collect();
        orphans.reverse();
        orphans
    }

    /// Every entry, newest first.
    fn drain(&mut self) -> Vec<Held> {
        self.held.drain(..).rev().map(|(_, held)| held).collect()
    }
}

impl Bot {
    pub(crate) fn claim(&mut self, held: Held) -> Claim {
        let claim = Claim(Arc::new(()));
        self.cancel.residue.held.push((Arc::downgrade(&claim.0), held));
        claim
    }

    /// Done with `claim` the usual way: releases it, unless a cancel already did.
    pub(crate) fn release(&mut self, claim: Claim) {
        if let Some(held) = self.cancel.residue.take(&claim) {
            self.release_held(held);
        }
    }

    /// Done with `claim` with nothing to undo (or handed on to an owner on the bot).
    pub(crate) fn unclaim(&mut self, claim: Claim) {
        self.cancel.residue.take(&claim);
    }

    /// Releases what dropped futures held.
    pub(crate) fn release_orphans(&mut self) {
        for held in self.cancel.residue.orphans() {
            self.release_held(held);
        }
    }

    /// Releases everything actions hold, owned or not.
    pub(crate) fn release_residue(&mut self) {
        for held in self.cancel.residue.drain() {
            self.release_held(held);
        }
    }

    fn release_held(&mut self, held: Held) {
        tracing::debug!(?held, "releasing an action's residue");
        match held {
            Held::Slot => self.governor.settled(),
            Held::Break(pos) => self.abort_break(pos),
            Held::InventoryScreen => self.close_own_inventory_screen(),
            Held::Jump(was) => {
                if let Some(c) = self.controls() {
                    c.jump = was;
                }
            }
            Held::Processes => {
                self.release_keys();
                self.forget_placements();
            }
            Held::Dismount => self.ride.dismount = false,
            Held::Bed => {
                if self.is_sleeping() {
                    self.leave_bed();
                }
            }
        }
    }
}
