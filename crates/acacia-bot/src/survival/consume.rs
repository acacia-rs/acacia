use acacia_client::proto::packets::{EntityEvent, EntityEventEventId};
use acacia_client::proto::types::{GameMode, InputData};

use super::food::{always_consumable, use_ticks};
use super::item_use::{eating_data, ItemUse, Snapshot, UseTick};
use crate::human::USE_RELEASE;
use crate::interact::wire;
use crate::{ActionError, Bot};

impl Bot {
    /// Eats or drinks the held item: holds "use" until the server has consumed it, then lets go
    /// (sending no `ReleaseItem`: survival/item_use.rs).
    /// Waits for the server's inventory or hunger update (`Rejected` if none comes). Dropping the
    /// future does not cancel the use: it finishes on its own as long as [`Bot::next`] is polled.
    pub async fn consume(&mut self) -> Result<(), ActionError> {
        self.start_item_use(false)?;
        loop {
            self.next_tick(|_, _| false).await?;
            if let Some(result) = self.survival.result.take() {
                return result;
            }
        }
    }

    /// Holding "use" on a food or drink (an explicit [`Bot::consume`] or auto-eat).
    pub fn is_using_item(&self) -> bool {
        self.survival.item_use.is_some()
    }

    pub(crate) fn start_item_use(&mut self, auto: bool) -> Result<(), ActionError> {
        if self.survival.item_use.is_some() {
            return Err(ActionError::NotPossible("already using an item".into()));
        }
        let held = self.state.inventory.held();
        let name = self.state.items.name(held.network_id).unwrap_or_default();
        let ticks = use_ticks(name).ok_or_else(|| ActionError::NotPossible(format!("cannot eat or drink {name:?}")))?;
        let p = &self.state.player;
        let hungry = p.hunger < 20.0 || !matches!(p.game_mode, GameMode::Survival | GameMode::Adventure);
        if !hungry && !always_consumable(name) {
            return Err(ActionError::NotPossible(format!("not hungry enough to eat {name}")));
        }
        let release = self.human.ticks_between(USE_RELEASE);
        let slot = self.state.inventory.selected_hotbar_slot;
        self.survival.item_use = Some(ItemUse::new(slot, self.use_snapshot(), ticks, release, auto));
        self.survival.result = None;
        self.client.send(&wire::click_air(self.hand()));
        self.queued_flags.push(InputData::StartUsingItem);
        self.set_using_item(true);
        Ok(())
    }

    /// Runs the item use one client tick on.
    pub(crate) fn tick_item_use(&mut self) {
        let snapshot = self.use_snapshot();
        let (selected, alive) = (self.state.inventory.selected_hotbar_slot, self.state.player.alive);
        let Some(item_use) = self.survival.item_use.as_mut() else { return };
        match item_use.tick(&snapshot, selected, alive) {
            UseTick::Nothing => {}
            UseTick::Effect => {
                let held = self.state.inventory.held();
                self.client.send(&EntityEvent {
                    runtime_entity_id: self.runtime_id(),
                    event_id: EntityEventEventId::EatingItem,
                    data: eating_data(held.network_id, held.metadata),
                    fire_at_position: None,
                });
            }
            UseTick::Finish => {
                self.client.send(&wire::finish_use(self.hand()));
            }
            UseTick::Done(result) => {
                let auto = item_use.auto;
                self.survival.item_use = None;
                self.set_using_item(false);
                if let Err(e) = &result {
                    tracing::debug!(error = %e, auto, "item use ended");
                }
                if !auto {
                    self.survival.result = Some(result);
                }
            }
        }
    }

    fn use_snapshot(&self) -> Snapshot {
        let held = self.state.inventory.held();
        let p = &self.state.player;
        Snapshot { network_id: held.network_id, count: held.count, hunger: p.hunger, saturation: p.saturation }
    }

    fn set_using_item(&mut self, on: bool) {
        if let Some(m) = self.movement.as_mut() {
            m.using_item = on;
        }
    }
}
