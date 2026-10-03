//! Packets the vanilla client sends on its own a moment after something happened: the end of a
//! block click, the late swing of an entity click, leaving a bed the server ended, resending the
//! held item when it changed. Counted down in client ticks. Timings: `human.rs`; shapes:
//! docs/research/survival-signs-beds.md §6.

use acacia_client::proto::packets::{Animate, AnimateActionId, ClientCameraAimAssist, ClientCameraAimAssistAction, InventoryContent};
use acacia_client::proto::types::Action;
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::BlockPos;

use crate::human::{Range, EQUIP_RESYNC, WAKE_REPLY};
use crate::interact::equipment::same_content;
use crate::interact::wire;
use crate::interact::SwingSource;
use crate::state::{Inventory, ItemStack};
use crate::Bot;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Reflex {
    /// `StopItemUseOn` at the click's result position.
    StopUseOn(BlockPos),
    Swing(SwingSource),
    StopSleeping,
    ClearAimAssist,
    /// Resend the held item if it still differs from the last `MobEquipment`.
    Equip,
}

#[derive(Debug, Default)]
pub(crate) struct Reflexes {
    due: Vec<(u32, Reflex)>,
    /// Hotbar slot and stack of the last `MobEquipment`; `None` until the inventory first arrives.
    pub equipped: Option<(u8, ItemStack)>,
    /// `StartSleeping` sent and no `StopSleeping` yet.
    pub in_bed: bool,
    /// The entity the last `Interact` mouse-over reported under the crosshair.
    pub hovered: Option<u64>,
    legacy_id: i32,
}

impl Reflexes {
    /// Schedules `reflex` in `ticks` ticks unless the same one is already waiting.
    pub fn schedule(&mut self, ticks: u32, reflex: Reflex) {
        if !self.is_scheduled(reflex) {
            self.due.push((ticks, reflex));
        }
    }

    pub fn is_scheduled(&self, reflex: Reflex) -> bool {
        self.due.iter().any(|(_, r)| *r == reflex)
    }

    /// Counts one tick down and returns what is due, in scheduling order.
    pub fn tick(&mut self) -> Vec<Reflex> {
        let mut fired = Vec::new();
        self.due.retain_mut(|(ticks, reflex)| match ticks.checked_sub(1) {
            None => {
                fired.push(*reflex);
                false
            }
            Some(left) => {
                *ticks = left;
                true
            }
        });
        fired
    }

    /// Vanilla's `legacy_request_id`s: negative and even on one counter (capture: -70, sign edits
    /// skip one each, -76, -78). The starting value is unknown.
    pub fn next_legacy_id(&mut self) -> i32 {
        self.legacy_id -= 2;
        self.legacy_id
    }
}

impl Bot {
    pub(crate) fn later(&mut self, range: Range, reflex: Reflex) {
        if !self.reflexes.is_scheduled(reflex) {
            let ticks = self.human.ticks_between(range);
            self.reflexes.schedule(ticks, reflex);
        }
    }

    /// Reacts to `packet` once the trackers have applied it.
    pub(crate) fn reflex_packet(&mut self, packet: &RawPacket) {
        match packet.id {
            Animate::ID => {
                let woken = packet.decode::<Animate>().is_ok_and(|a| a.action_id == AnimateActionId::WakeUp && a.runtime_entity_id == self.runtime_id());
                if woken {
                    self.leave_bed_later();
                }
            }
            // The first inventory is what the client starts holding; it sends no MobEquipment for it.
            InventoryContent::ID
                if self.reflexes.equipped.is_none()
                    && packet.decode::<InventoryContent>().is_ok_and(|p| p.window_id.to_raw() as i32 == Inventory::WINDOW_INVENTORY) =>
            {
                let inv = &self.state.inventory;
                self.reflexes.equipped = Some((inv.selected_hotbar_slot, inv.held().clone()));
            }
            _ => {}
        }
    }

    /// The server ended the sleep (WakeUp, or the flag cleared): vanilla answers with StopSleeping.
    fn leave_bed_later(&mut self) {
        if std::mem::take(&mut self.reflexes.in_bed) {
            self.later(WAKE_REPLY, Reflex::StopSleeping);
        }
    }

    pub(crate) fn tick_reflexes(&mut self) {
        if self.reflexes.in_bed && !self.is_sleeping() {
            self.leave_bed_later();
        }
        if self.held_changed() {
            self.later(EQUIP_RESYNC, Reflex::Equip);
        }
        for reflex in self.reflexes.tick() {
            self.fire(reflex);
        }
    }

    fn held_changed(&self) -> bool {
        let inv = &self.state.inventory;
        self.reflexes.equipped.as_ref().is_some_and(|(slot, item)| *slot != inv.selected_hotbar_slot || !same_content(item, inv.held()))
    }

    fn fire(&mut self, reflex: Reflex) {
        let me = self.runtime_id();
        match reflex {
            Reflex::StopUseOn(pos) => {
                self.client.send(&wire::item_use_on(me, Action::StopItemUseOn, pos, [0, 0, 0], 0));
            }
            Reflex::Swing(source) => self.swing_from(Some(source)),
            Reflex::StopSleeping => {
                self.client.send(&crate::sleep::sleep_action(me, Action::StopSleeping));
                self.reflexes.schedule(0, Reflex::ClearAimAssist);
            }
            Reflex::ClearAimAssist => {
                self.client.send(&clear_aim_assist());
            }
            Reflex::Equip if self.held_changed() => self.equip_now(),
            Reflex::Equip => {}
        }
    }
}

/// Sent after waking (and at spawn, mounts, teleports: vanilla-actions-2026-10-02.md).
pub(crate) fn clear_aim_assist() -> ClientCameraAimAssist {
    ClientCameraAimAssist { preset_id: String::new(), action: ClientCameraAimAssistAction::Clear, allow_aim_assist: false }
}

#[cfg(test)]
mod tests;
