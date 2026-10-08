use super::geometry::{entity_aim, ENTITY_REACH};
use super::wire::{self, SwingSource};
use crate::human::{ENTITY_HOVER, ENTITY_SWING};
use crate::reflex::Reflex;
use crate::{ActionError, Bot};

impl Bot {
    /// Attacks a tracked entity (needs `Trackers::entities`): faces it, swings and
    /// sends `UseItemOnEntity` Attack. `NotPossible` beyond [`ENTITY_REACH`] from the eye.
    /// Attack cooldown is up to the caller.
    pub async fn attack(&mut self, runtime_id: u64) -> Result<(), ActionError> {
        self.use_on_entity(runtime_id, true).await
    }

    /// Right-clicks a tracked entity (`UseItemOnEntity` Interact): trade, ride, feed, name...
    /// The swing follows on its own 120-200 ms later, as vanilla's does.
    pub async fn interact_entity(&mut self, runtime_id: u64) -> Result<(), ActionError> {
        self.use_on_entity(runtime_id, false).await
    }

    /// Vanilla clicks an entity only after reporting the crosshair on it (`Interact` MouseOverEntity
    /// with the hit point, 75-1277 ms before in the capture).
    async fn use_on_entity(&mut self, runtime_id: u64, attack: bool) -> Result<(), ActionError> {
        let (aim, _) = self.entity_in_reach(runtime_id)?;
        self.look_at(aim).await?;
        if self.reflexes.hovered != Some(runtime_id) {
            let (_, hit) = self.entity_in_reach(runtime_id)?;
            self.client.send(&wire::mouse_over(runtime_id, hit));
            self.reflexes.hovered = Some(runtime_id);
            let pause = self.human.between(ENTITY_HOVER);
            self.pause(pause).await?;
        }
        // The target may have moved while the rotation went out.
        self.click_entity_aimed(runtime_id, attack)
    }

    /// Left- (`attack`) or right-clicks a tracked entity now, with the current aim: reports the
    /// crosshair on it if it was elsewhere, swings and sends `UseItemOnEntity`.
    pub fn click_entity_aimed(&mut self, runtime_id: u64, attack: bool) -> Result<(), ActionError> {
        let (_, hit) = self.entity_in_reach(runtime_id)?;
        if self.reflexes.hovered != Some(runtime_id) {
            self.client.send(&wire::mouse_over(runtime_id, hit));
            self.reflexes.hovered = Some(runtime_id);
        }
        tracing::debug!(runtime_id, attack, ?hit, eye = ?self.eye_position(), "entity click");
        if attack {
            self.swing_from(Some(SwingSource::Attack));
        }
        self.client.send(&wire::use_on_entity(self.hand(), runtime_id, attack, hit));
        if !attack {
            self.later(ENTITY_SWING, Reflex::Swing(SwingSource::Interact));
        }
        Ok(())
    }

    /// Aim point and hit point on the entity, if it is tracked and within reach.
    fn entity_in_reach(&self, runtime_id: u64) -> Result<([f32; 3], [f32; 3]), ActionError> {
        let entity = self.state.entities.get(runtime_id).ok_or_else(|| {
            ActionError::NotPossible(format!("entity {runtime_id} is not tracked (is Trackers::entities on?)"))
        })?;
        let (aim, hit, distance) = entity_aim(self.eye_position(), entity);
        if distance > ENTITY_REACH {
            return Err(ActionError::NotPossible(format!("entity {runtime_id} is {distance:.2} blocks away")));
        }
        Ok((aim, hit))
    }
}
