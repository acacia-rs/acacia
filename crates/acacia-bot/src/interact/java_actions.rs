//! Block actions on Java, as `MultiPlayerGameMode` sends them. Digging: `START_DESTROY_BLOCK`, then each tick the
//! block's [`java_progress`] summed in f32 until it reaches 1 and `STOP_DESTROY_BLOCK` goes out, with a punch every
//! tick. The server's acknowledgement of an action's sequence is its echo.

use acacia_java_proto::actions::{Direction, Hand};
use acacia_physics::{BlockPos, Vec3};
use acacia_world::Mining;

use super::break_time::java_progress;
use super::Face;
use crate::bot::Backend;
use crate::{ActionError, Bot};

pub(crate) fn direction(face: Face) -> Direction {
    Direction::from_index(face as i32).unwrap_or_default()
}

impl Bot {
    /// [`Bot::mine`] on Java; `Ok(true)` if the block disappeared on its own first.
    pub(super) async fn java_mine(&mut self, pos: BlockPos, face: Face, mining: Mining) -> Result<bool, ActionError> {
        let progress = |bot: &Bot| java_progress(&mining, &bot.break_conditions()).unwrap_or(0.0);
        let breaks = progress(self) >= 1.0;
        self.java_send(|c| c.start_destroy(pos, direction(face), breaks))?;
        self.java_send(|c| c.punch().map(|()| 0))?;
        self.next_tick(|_, _| false).await?;
        if breaks {
            return Ok(false);
        }
        let mut done = 0.0_f32;
        loop {
            if self.block_at(pos).is_some_and(|(_, s)| s.is_air()) {
                return Ok(true);
            }
            done += progress(self);
            if done >= 1.0 {
                self.java_send(|c| c.stop_destroy(pos, direction(face)))?;
                self.java_send(|c| c.punch().map(|()| 0))?;
                self.next_tick(|_, _| false).await?;
                return Ok(false);
            }
            self.java_send(|c| c.punch().map(|()| 0))?;
            self.next_tick(|_, _| false).await?;
        }
    }

    /// Sends through the Java client, keeping the sequence of a predicting action (non-zero) as the next echo to
    /// wait for.
    fn java_send(&mut self, send: impl FnOnce(&mut acacia_java_client::Client) -> Result<i32, acacia_java_client::Error>) -> Result<(), ActionError> {
        let Backend::Java(java) = &mut self.backend else { return Err(ActionError::Unsupported("Java actions")) };
        let sequence = send(&mut java.client).map_err(|_| ActionError::Disconnected)?;
        if sequence != 0 {
            java.awaiting = sequence;
        }
        Ok(())
    }

    /// The server answered the last predicting action: its block updates for the cell came before the answer.
    pub(crate) fn java_echoed(&mut self) -> bool {
        let Backend::Java(java) = &mut self.backend else { return false };
        let answered = java.awaiting != 0 && java.client.actions.acked() >= java.awaiting;
        if answered {
            java.awaiting = 0;
        }
        answered
    }

    /// `useItemOn` with the main hand at `click` (relative to `pos`'s corner); the held stack loses one when placing.
    pub(super) fn java_click_block(&mut self, pos: BlockPos, face: Face, click: Vec3, placing: bool) -> Result<(), ActionError> {
        self.java_send(|c| c.use_item_on(Hand::Main, pos, direction(face), click))?;
        if placing {
            let slot = usize::from(self.state.inventory.selected_hotbar_slot);
            let held = &mut self.state.inventory.main[slot];
            *held = super::item_use::one_used(held);
        }
        Ok(())
    }

    pub(crate) fn java_abort_break(&mut self, pos: BlockPos, face: Face) {
        if let Backend::Java(java) = &mut self.backend
            && let Err(e) = java.client.abort_destroy(pos, direction(face))
        {
            tracing::debug!(error = %e, "abort not sent");
        }
    }
}
