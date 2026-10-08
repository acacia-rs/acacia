use std::time::Duration;

use acacia_client::proto::types::Action;
use acacia_client::proto::RawPacket;
use acacia_physics::BlockPos;

use super::geometry::{block_distance, face_point, BLOCK_REACH};
use super::legacy::{self, SlotChange};
use super::wire::{self, SwingSource};
use super::Face;
use crate::human::STOP_USE_ON;
use crate::reflex::Reflex;
use crate::state::ItemStack;
use crate::{ActionError, Bot};

/// How long `place_block` waits for the server to show the placed block.
const PLACE_TIMEOUT: Duration = Duration::from_secs(1);

impl Bot {
    /// Swings the main hand (`Animate`), as a click on nothing does.
    pub fn swing(&mut self) {
        self.swing_from(None);
    }

    pub(crate) fn swing_from(&mut self, source: Option<SwingSource>) {
        self.client.send(&wire::swing(self.runtime_id(), source));
    }

    /// Uses the held item in the air (`UseItem` ClickAir): eat, drink, throw, draw a bow, raise a
    /// shield. Food finishes on its own; bows and crossbows need [`Bot::release_item`]. Geyser
    /// aims throws with the rotation of the last `PlayerAuthInput`, so physics bots should set
    /// [`crate::movement::Controls`] a tick before.
    pub fn use_item(&mut self) {
        self.client.send(&wire::click_air(self.hand()));
    }

    /// Releases the item in use (`ReleaseItem` Release): shoots a drawn bow.
    pub fn release_item(&mut self) {
        self.client.send(&wire::release(self.hand()));
    }

    /// Right-clicks `face` of the block at `pos` with the held item (`UseItem` ClickBlock): opens
    /// doors, presses buttons, uses containers. Physics bots face the block first.
    pub async fn use_item_on_block(&mut self, pos: BlockPos, face: Face) -> Result<(), ActionError> {
        let distance = block_distance(self.eye_position(), pos);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("block {pos:?} is {distance:.2} blocks away")));
        }
        self.look_at(face_point(pos, face)).await?;
        self.click_block(pos, face, SwingSource::Interact)
    }

    /// Places the held block or item (sign, bed, boat...) against `face` of the block at `against`.
    /// Physics bots wait until the server shows a block in the target cell (`Rejected` if it does
    /// not within a second; items that make entities, like boats, are not waited for).
    pub async fn place_block(&mut self, against: BlockPos, face: Face) -> Result<(), ActionError> {
        if self.state.inventory.held().is_empty() {
            return Err(ActionError::NotPossible("nothing held to place".into()));
        }
        let target = face.adjacent(against);
        if let Some((_, s)) = self.block_at(target)
            && !s.is_air()
            && !s.is_liquid()
        {
            return Err(ActionError::NotPossible(format!("{target:?} is occupied by {}", s.name)));
        }
        if self.block_at(against).is_some_and(|(_, s)| s.is_air()) {
            return Err(ActionError::NotPossible(format!("nothing to place against at {against:?}")));
        }
        let distance = block_distance(self.eye_position(), against);
        if distance > BLOCK_REACH {
            return Err(ActionError::NotPossible(format!("block {against:?} is {distance:.2} blocks away")));
        }
        self.look_at(face_point(against, face)).await?;
        let makes_entity = self.held_name().is_some_and(|n| n.ends_with("boat") || n.ends_with("minecart"));
        self.click_block(against, face, SwingSource::Build)?;
        if makes_entity || self.block_at(target).is_none() {
            return Ok(());
        }
        let placed = |bot: &Bot, _: &RawPacket| bot.block_at(target).is_some_and(|(_, s)| !s.is_air() && !s.is_liquid()).then_some(());
        match self.wait_until(PLACE_TIMEOUT, placed).await {
            Err(ActionError::Timeout) => Err(ActionError::Rejected(format!("no block appeared at {target:?}"))),
            other => other,
        }
    }

    pub(super) fn held_name(&self) -> Option<&str> {
        self.state.items.name(self.state.inventory.held().network_id)
    }

    /// Vanilla's block click: StartItemUseOn, the swing, the ClickBlock (placing: with the held
    /// stack's predicted change, then the new stack in MobEquipment), StopItemUseOn a moment later.
    pub(super) fn click_block(&mut self, pos: BlockPos, face: Face, source: SwingSource) -> Result<(), ActionError> {
        let block = self.block_wire_id(pos)?;
        let held = self.state.inventory.held().clone();
        let is_sign = self.held_name().is_some_and(|n| n.ends_with("sign"));
        let result = use_on_result(&held, is_sign, pos, face);
        if self.reflexes.hovered.take().is_some() {
            self.client.send(&wire::mouse_over_nothing());
        }
        self.client.send(&wire::item_use_on(self.runtime_id(), Action::StartItemUseOn, pos, result, face as i32));
        self.swing_from(Some(source));
        let packet = wire::click_block(self.hand(), pos, face, face.click_offset(), block);
        if source != SwingSource::Build {
            self.client.send(&packet);
        } else {
            let slot = self.state.inventory.selected_hotbar_slot;
            let left = one_used(&held);
            // Capture: sign 2 -> 1 had a legacy id, bed and boat 1 -> 0 had none.
            let legacy_id = (!left.is_empty()).then(|| self.reflexes.next_legacy_id());
            self.client.send(&legacy::with_slot_change(packet, legacy_id, &SlotChange { slot, old: &held, new: &left }));
            self.state.inventory.main[usize::from(slot)] = left;
            if is_sign {
                self.client.send(&wire::click_air(self.hand()));
            }
            self.equip_now();
        }
        self.later(STOP_USE_ON, Reflex::StopUseOn(result));
        Ok(())
    }
}

/// Vanilla's `result_position`: the cell a placement would fill when the hand is empty or holds a
/// block or sign, else the clicked block (beds, boats, tools, other items).
pub(crate) fn use_on_result(held: &ItemStack, is_sign: bool, pos: BlockPos, face: Face) -> BlockPos {
    if held.is_empty() || held.block_runtime_id != 0 || is_sign {
        face.adjacent(pos)
    } else {
        pos
    }
}

/// The stack left after placing one.
pub(crate) fn one_used(held: &ItemStack) -> ItemStack {
    match held.count {
        0 | 1 => ItemStack::default(),
        n => ItemStack { count: n - 1, ..held.clone() },
    }
}
