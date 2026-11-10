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

impl Bot {
    /// `UseItem` with the main hand, sent with the next tick; nothing on Bedrock.
    pub(crate) fn java_use_item(&mut self) {
        if let Backend::Java(java) = &mut self.backend
            && let Err(e) = java.client.use_item(Hand::Main)
        {
            tracing::debug!(error = %e, "java use item not queued");
        }
    }

    /// `RELEASE_USE_ITEM`, sent with the next tick; nothing on Bedrock.
    pub(crate) fn java_release_item(&mut self) {
        if let Backend::Java(java) = &mut self.backend
            && let Err(e) = java.client.release_use_item()
        {
            tracing::debug!(error = %e, "java release not queued");
        }
        self.set_java_using_item(false);
    }

    /// `ATTACK` then the swing (`gameMode.attack`, `player.swing`), or `INTERACT` at `hit` relative to the
    /// entity's feet with the swing left to [`crate::reflex::Reflex::Swing`]; queued for the next tick.
    pub(super) fn java_click_entity(&mut self, runtime_id: u64, attack: bool, hit: [f32; 3]) -> Result<(), ActionError> {
        let feet = self.state.entities.get(runtime_id).map(|e| e.feet()).ok_or(ActionError::NotPossible("entity left".into()))?;
        let entity = i32::try_from(runtime_id).map_err(|_| ActionError::NotPossible(format!("entity {runtime_id} is not a Java id")))?;
        let Backend::Java(java) = &mut self.backend else { return Err(ActionError::Unsupported("Java actions")) };
        let sent = if attack {
            java.client.attack(entity).and_then(|()| java.client.punch())
        } else {
            let at = [hit[0] - feet.x, hit[1] - feet.y, hit[2] - feet.z].map(f64::from);
            java.client.interact(entity, Hand::Main, at, false)
        };
        sent.map_err(|_| ActionError::Disconnected)
    }

    /// `SWING` (Java's punch packet), sent with the next tick; nothing on Bedrock.
    pub(crate) fn java_swing(&mut self) {
        if let Backend::Java(java) = &mut self.backend
            && let Err(e) = java.client.punch()
        {
            tracing::debug!(error = %e, "java swing not queued");
        }
    }

    /// The physics' using-item slowdown: vanilla's default `use_effects` (0.2 speed, no sprinting).
    pub(crate) fn set_java_using_item(&mut self, on: bool) {
        if let Backend::Java(java) = &mut self.backend {
            let using = acacia_java_physics::ItemUse { speed: 0.2, can_sprint: false };
            java.client.movement.player.using_item = on.then_some(using);
        }
    }
}
