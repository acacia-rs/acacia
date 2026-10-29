//! Cancelling the action in flight, and cleaning up after a cancelled or dropped one. This is the one place the
//! contract is written down; actions point here.
//!
//! - [`CancelHandle::cancel`] (from any thread) cancels the action in flight when it is called. Every action waits
//!   through one step (`Bot::progress`), which then fails with [`ActionError::Cancelled`] within a tick, and keeps
//!   failing so until the caller takes the bot back by polling [`Bot::next`] or calling [`Bot::abandon_action`]: an
//!   action that retries its failures (a build, a goto re-planning) cannot carry on. A cancel while no action waits
//!   is ignored. Two actions awaited back to back, with neither call between them, count as one.
//! - Actions record on the bot what they take (residue.rs: governor slots, a block being mined, the own inventory
//!   screen, held keys, a dismount or a bed in progress) and clear it when they finish. A cancel releases all of it
//!   before `Cancelled` comes back: slots settled, `AbortBreak` sent, the screen closed, movement keys (sneak too)
//!   let go. A future dropped mid-action (`select!`, `timeout`) needs no call either: what it held is released at
//!   the bot's next wait or [`Bot::next`]; [`Bot::abandon_action`] releases it at once.
//! - Not undone: an item already consumed, a request the server already took, an A* search running on the blocking
//!   pool (it finishes and is discarded), a container the caller opened.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{ActionError, Bot};

mod residue;

pub(crate) use residue::{Claim, Held};

/// Cancels the bot's action in flight, from any thread (see [`crate::cancel`]). Cheap to clone.
#[derive(Debug, Clone)]
pub struct CancelHandle(Arc<Shared>);

#[derive(Debug)]
struct Shared {
    /// Bumped when an action starts and when the caller takes the bot back, so idle spells and actions alternate.
    epoch: AtomicU64,
    /// The latest epoch a cancel was asked in.
    cancelled: AtomicU64,
}

impl CancelHandle {
    /// Cancels the action in flight now; later actions are not affected.
    pub fn cancel(&self) {
        let epoch = self.0.epoch.load(Ordering::Acquire);
        self.0.cancelled.fetch_max(epoch, Ordering::AcqRel);
    }
}

/// The bot's side of [`CancelHandle`].
#[derive(Debug)]
pub(crate) struct CancelState {
    shared: Arc<Shared>,
    /// An action has waited since the caller last took the bot back.
    acting: bool,
    residue: residue::Residue,
}

impl Default for CancelState {
    fn default() -> Self {
        let shared = Arc::new(Shared { epoch: AtomicU64::new(1), cancelled: AtomicU64::new(0) });
        Self { shared, acting: false, residue: Default::default() }
    }
}

impl CancelState {
    fn begin(&mut self) {
        if !std::mem::replace(&mut self.acting, true) {
            self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// True if an action was running.
    fn end(&mut self) -> bool {
        let acting = std::mem::take(&mut self.acting);
        if acting {
            self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        }
        acting
    }

    fn requested(&self) -> bool {
        self.acting && self.shared.cancelled.load(Ordering::Acquire) == self.shared.epoch.load(Ordering::Acquire)
    }
}

impl Bot {
    /// A handle that cancels this bot's action in flight (see [`crate::cancel`]).
    pub fn cancel_handle(&self) -> CancelHandle {
        CancelHandle(self.cancel.shared.clone())
    }

    /// Every action wait passes here: notes an action starting, releases what dropped futures held, and fails a
    /// cancelled action after releasing everything it holds.
    pub(crate) fn cancel_point(&mut self) -> Result<(), ActionError> {
        self.cancel.begin();
        self.release_orphans();
        if self.cancel.requested() {
            self.release_residue();
            self.release_keys();
            return Err(ActionError::Cancelled);
        }
        Ok(())
    }

    /// Ends the action in flight or dropped: releases what it held, lets go of the movement keys, gives up on
    /// pipelined placements and clears a cancel (see [`crate::cancel`]). Idempotent.
    pub fn abandon_action(&mut self) {
        self.release_residue();
        self.release_keys();
        self.forget_placements();
        self.cancel.end();
    }

    /// `next` took the bot back: no action can still own anything.
    pub(crate) fn idle_again(&mut self) {
        if self.cancel.end() {
            self.release_residue();
        }
    }

    pub(crate) fn release_keys(&mut self) {
        if let Some(c) = self.controls() {
            c.stop();
            c.sneak = false;
        }
    }
}

#[cfg(test)]
mod tests;
