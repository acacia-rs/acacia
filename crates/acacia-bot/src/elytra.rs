//! Elytra actions: equipping one, starting and stopping a glide, firework boosts (input shapes from the
//! 2026-10-02 vanilla capture). The glide itself is simulated (`Controls::glide`); its Start/StopGliding
//! flags come from the simulation's transitions, landing included.

use std::time::Duration;

use acacia_client::proto::codec::read_varint64;
use acacia_client::proto::packets::SetEntityData;
use acacia_client::proto::types::{MetadataDictionaryItemValue, MetadataFlags1};
use acacia_client::proto::{Packet, RawPacket};

use crate::human::HOTBAR_SWITCH;
use crate::survival::Destination;
use crate::state::Inventory;
use crate::{ActionError, Bot};

const ELYTRA: &str = "minecraft:elytra";
const FIREWORK: &str = "minecraft:firework_rocket";
/// Armor slot of the chestplate.
const CHEST: u8 = 1;
/// How long the server has to confirm a glide with the `GLIDING` actor flag.
const GLIDE_TIMEOUT: Duration = Duration::from_secs(1);

impl Bot {
    /// Puts an elytra from the main inventory on (swapping out any chestplate). No-op when one is worn.
    pub async fn equip_elytra(&mut self) -> Result<(), ActionError> {
        if self.wears_elytra() {
            return Ok(());
        }
        let from = self.find_item(ELYTRA).ok_or_else(|| ActionError::NotPossible("no elytra in the inventory".into()))?;
        self.equip(from, Destination::Chest).await
    }

    pub fn wears_elytra(&self) -> bool {
        let id = self.state.items.id(ELYTRA);
        self.state.inventory.armor.get(usize::from(CHEST)).is_some_and(|s| !s.is_empty() && Some(s.network_id) == id)
    }

    /// Opens the elytra mid-air as vanilla does on a jump press while falling: the press plus
    /// `StartGliding` and `WantUp` in the next `PlayerAuthInput`, no `PlayerAction`. Waits for the
    /// server's `GLIDING` flag (`Rejected` without it). Physics bots only, airborne with an elytra worn.
    pub async fn start_gliding(&mut self) -> Result<(), ActionError> {
        let Some(movement) = self.movement.as_ref().filter(|m| m.is_started()) else {
            return Err(ActionError::NotPossible("gliding needs a physics bot".into()));
        };
        if movement.on_ground() {
            return Err(ActionError::NotPossible("on the ground".into()));
        }
        if !self.wears_elytra() {
            return Err(ActionError::NotPossible("no elytra worn".into()));
        }
        self.set_glide(true);
        self.press_jump().await?;
        let me = self.state.player.runtime_entity_id;
        match self.wait_until(GLIDE_TIMEOUT, |_, p| gliding_flag(p, me).filter(|&g| g)).await {
            Err(ActionError::Timeout) => {
                self.set_glide(false);
                Err(ActionError::Rejected("the server did not start the glide".into()))
            }
            other => other.map(drop),
        }
    }

    /// Closes the elytra (physics bots only). In the air vanilla does it on a jump press (`StopGliding`
    /// + the press flags); landing ends a glide by itself.
    pub async fn stop_gliding(&mut self) -> Result<(), ActionError> {
        let Some(movement) = self.movement.as_ref().filter(|m| m.is_started()) else {
            return Err(ActionError::NotPossible("gliding needs a physics bot".into()));
        };
        let airborne = !movement.on_ground();
        self.set_glide(false);
        if airborne {
            return self.press_jump().await;
        }
        self.next_tick(|_, _| false).await.map(drop)
    }

    /// Whether the simulation is gliding (physics bots).
    pub fn is_gliding(&self) -> bool {
        self.movement.as_ref().is_some_and(|m| m.gliding())
    }

    /// Uses a firework rocket from the hotbar like vanilla: `Animate` "useitem" + `UseItem` ClickAir,
    /// no `StartUsingItem`. Selecting the rocket first takes a human pause.
    pub async fn boost_with_firework(&mut self) -> Result<(), ActionError> {
        let id = self.state.items.id(FIREWORK);
        let hotbar = self.state.inventory.main.iter().take(usize::from(Inventory::HOTBAR_SLOTS));
        let slot = id.and_then(|id| hotbar.clone().position(|s| s.network_id == id && !s.is_empty()));
        let slot = slot.ok_or_else(|| ActionError::NotPossible("no firework rocket in the hotbar".into()))?;
        if usize::from(self.state.inventory.selected_hotbar_slot) != slot {
            self.select_hotbar(slot as u8)?;
            let delay = self.human.between(HOTBAR_SWITCH);
            self.pause(delay).await?;
        }
        self.use_item_like_vanilla();
        if let Some(movement) = self.movement.as_mut() {
            movement.predict_glide_boost();
        }
        Ok(())
    }

    fn set_glide(&mut self, on: bool) {
        if let Some(movement) = self.movement.as_mut() {
            movement.controls.glide = on;
        }
    }

    /// Holds jump for one tick, then releases it.
    async fn press_jump(&mut self) -> Result<(), ActionError> {
        let Some(movement) = self.movement.as_mut() else { return Ok(()) };
        let held = movement.controls.jump;
        movement.controls.jump = true;
        let ticked = self.next_tick(|_, _| false).await;
        if let Some(movement) = self.movement.as_mut() {
            movement.controls.jump = held;
        }
        ticked.map(drop)
    }
}

/// The `GLIDING` flag if `packet` is a `SetEntityData` for `runtime_id` that carries the flags.
pub(crate) fn gliding_flag(packet: &RawPacket, runtime_id: u64) -> Option<bool> {
    if packet.id != SetEntityData::ID || read_varint64(&mut &packet.body[..]).ok()? != runtime_id {
        return None;
    }
    packet.decode::<SetEntityData>().ok()?.metadata.into_iter().find_map(|m| match m.value {
        MetadataDictionaryItemValue::Flags(f) => Some(f.0 & MetadataFlags1::GLIDING.0 != 0),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use acacia_client::proto::types::{EntityProperties, MetadataDictionaryItem, MetadataDictionaryItemKey, MetadataDictionaryItemType};

    use super::*;
    use crate::state::queries::test_support::raw;

    fn flags(runtime_id: u64, flags: i64) -> RawPacket {
        let item = MetadataDictionaryItem {
            key: MetadataDictionaryItemKey::Flags,
            r#type: MetadataDictionaryItemType::Long,
            legacy_type: 0,
            value: MetadataDictionaryItemValue::Flags(MetadataFlags1(flags)),
        };
        raw(&SetEntityData { runtime_entity_id: runtime_id, metadata: vec![item], properties: EntityProperties { ints: vec![], floats: vec![] }, tick: 0 })
    }

    #[test]
    fn reads_the_own_gliding_flag() {
        assert_eq!(gliding_flag(&flags(3, MetadataFlags1::GLIDING.0 | 1), 3), Some(true));
        assert_eq!(gliding_flag(&flags(3, 1), 3), Some(false));
        assert_eq!(gliding_flag(&flags(4, MetadataFlags1::GLIDING.0), 3), None);
    }
}
