//! Elytra on Java: the physics opens the glide on a jump press in the air (`tryToStartFallFlying`) and sends
//! `START_FALL_FLYING`; only landing or water ends it, as Java has no mid-air close.

use std::time::Duration;

use crate::{ActionError, Bot};

/// How long the server has to keep the glide before it counts as accepted: a refusal clears the flag.
const GLIDE_SETTLE: Duration = Duration::from_millis(250);

impl Bot {
    pub(super) async fn java_start_gliding(&mut self) -> Result<(), ActionError> {
        let player = &self.java().ok_or(ActionError::Disconnected)?.client().movement.player;
        if player.on_ground {
            return Err(ActionError::NotPossible("on the ground".into()));
        }
        if !player.has_glider {
            return Err(ActionError::NotPossible("no usable elytra worn".into()));
        }
        self.press_jump().await?;
        self.pause(GLIDE_SETTLE).await?;
        if self.is_gliding() { Ok(()) } else { Err(ActionError::Rejected("the server did not start the glide".into())) }
    }

    pub(super) async fn java_stop_gliding(&mut self) -> Result<(), ActionError> {
        if self.is_gliding() && !self.java().is_some_and(|j| j.client().movement.player.on_ground) {
            return Err(ActionError::NotPossible("Java elytras close only on landing or in water".into()));
        }
        self.next_tick(|_, _| false).await.map(drop)
    }
}
