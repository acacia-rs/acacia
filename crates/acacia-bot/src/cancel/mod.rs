//! Cancelling the action in flight. This is the one place the contract is written down; actions point here.
//!
//! - [`CancelHandle::cancel`] (from any thread) cancels the action in flight when it is called. Every action waits
//!   through one step (`Bot::progress`), which then fails with [`ActionError::Cancelled`] within a tick, and keeps
//!   failing so until the caller takes the bot back by polling [`Bot::next`] or calling [`Bot::abandon_action`]: an
//!   action that retries its failures (a build, a goto re-planning) cannot carry on. A cancel while no action waits
//!   is ignored. Two actions awaited back to back, with neither call between them, count as one.
//! - What a cancelled or dropped action took (governor slots, a block being mined, the own inventory screen, held
//!   keys) is not released yet.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{ActionError, Bot};

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
}

impl Default for CancelState {
    fn default() -> Self {
        Self { shared: Arc::new(Shared { epoch: AtomicU64::new(1), cancelled: AtomicU64::new(0) }), acting: false }
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

    /// Every action wait passes here: notes an action starting, and fails a cancelled one.
    pub(crate) fn cancel_point(&mut self) -> Result<(), ActionError> {
        self.cancel.begin();
        if self.cancel.requested() {
            return Err(ActionError::Cancelled);
        }
        Ok(())
    }

    /// Ends the action in flight or dropped and clears a cancel: the caller has the bot back (see [`crate::cancel`]).
    /// Idempotent; [`Bot::next`] does it too.
    pub fn abandon_action(&mut self) {
        self.cancel.end();
    }

    /// `next` took the bot back.
    pub(crate) fn idle_again(&mut self) {
        self.cancel.end();
    }
}

#[cfg(test)]
mod tests;
