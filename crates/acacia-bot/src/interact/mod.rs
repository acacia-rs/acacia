//! World interactions: hotbar, swings, entities, item use, block placing, containers and
//! server-authoritative block breaking. Packet semantics: docs/research/04-bedrock-gameplay-layer.md §4.
//!
//! Wire choices (gophertunnel for layout; Geyser `BedrockInventoryTransactionTranslator` /
//! `BlockBreakHandler` and Boar `ItemTransactionValidator` / `Reach` for what is checked):
//! - `hotbar_slot` / `held_item` are the tracked selection: Geyser switches the Java slot to
//!   `hotbar_slot` and rejects a ClickBlock whose held item type differs from that slot; Boar compares
//!   it with the item from the last `MobEquipment`.
//! - `player_pos` is the eye position, like `PlayerAuthInput.position`. Entity `click_pos` is the world
//!   point hit (Geyser subtracts the entity position); block `click_pos` is relative to the block.
//! - ClickAir uses block position (0,0,0) (Boar rejects anything else further than 12 blocks) and face 255.
//! - Block ids in `block_runtime_id` use the server's id form (hashed on BDS, runtime on Geyser).
//!
//! Packet order and the extra packets around clicks (StartItemUseOn/StopItemUseOn, held-stack changes,
//! late entity swing) follow a vanilla capture: docs/research/survival-signs-beds.md §6.
//!
//! Aim: rotation only reaches the server in `PlayerAuthInput`, so bots turn and let one tick pass
//! before aim-sensitive packets ([`Bot::look_at`]); Boar ray-casts attacks from the rotation it last
//! received and re-checks after the next input, BDS checks a mount click against the look ray.

mod break_time;
pub(crate) mod breaking;
mod container;
mod direct;
mod entity;
pub(crate) mod equipment;
mod geometry;
mod item_use;
pub(crate) mod legacy;
pub(crate) mod wire;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vanilla_tests;

pub use break_time::{break_ticks, BreakConditions};
pub use direct::is_interactive;
pub(crate) use direct::Mining;
pub use geometry::{facing_face, Face, BLOCK_REACH, ENTITY_REACH};
pub use wire::{to_wire, SwingSource};

use acacia_physics::{BlockPos, Vec3};
use acacia_world::{BlockAccess, BlockState};

use crate::{ActionError, Bot};

/// Rotation changes below this (degrees) need no extra tick before acting.
const AIM_TOLERANCE: f32 = 0.5;

impl Bot {
    /// Eye position: simulated for physics bots, last server position otherwise.
    pub fn eye_position(&self) -> Vec3 {
        if let Some(eye) = self.movement.as_ref().and_then(|m| m.eye_position()) {
            return eye;
        }
        let e = self.state.player.eye_position();
        [e.x, e.y, e.z]
    }

    pub(crate) fn has_physics(&self) -> bool {
        self.movement.as_ref().is_some_and(|m| m.is_started())
    }

    pub(crate) fn hand(&self) -> wire::Hand {
        let inv = &self.state.inventory;
        wire::Hand { slot: inv.selected_hotbar_slot, item: to_wire(inv.held()), eye: self.eye_position() }
    }

    pub(crate) fn runtime_id(&self) -> u64 {
        self.state.player.runtime_entity_id
    }

    /// Runtime id and state of the tracked block at `pos` (idle bots: only right around them).
    pub(crate) fn block_at(&self, [x, y, z]: BlockPos) -> Option<(u32, &BlockState)> {
        let world = self.world.as_ref().filter(|w| w.knows_block(x, y, z))?;
        let (view, registry) = (world.view()?, world.registry()?);
        let id = view.block(x, y, z);
        registry.get(id).map(|s| (id, s))
    }

    /// The id the server uses for the block at `pos`. BDS ignores a click naming another block, so an
    /// unknown block is refused rather than clicked blind.
    fn block_wire_id(&self, pos: BlockPos) -> Result<u32, ActionError> {
        match (self.world.as_ref().and_then(|w| w.view()), self.block_at(pos)) {
            (Some(view), Some((id, _))) => Ok(view.world().wire_id(id)),
            _ => Err(ActionError::NotPossible(format!("block {pos:?} is not known (not loaded, or not received near an idle bot)"))),
        }
    }

    /// (yaw, pitch) the bot's inputs report: its own aim, or for an idle bot that has not aimed, the
    /// server's last rotation.
    pub fn facing(&self) -> (f32, f32) {
        match (self.movement.as_ref(), self.idle.as_ref()) {
            (Some(m), _) => (m.controls.yaw, m.controls.pitch),
            (None, Some((idle, _))) => idle.facing(&self.state.player),
            (None, None) => (self.state.player.yaw, self.state.player.pitch),
        }
    }

    /// Turns towards `target` and, if the rotation changed, lets one tick pass so the next
    /// `PlayerAuthInput` has carried it to the server. No-op before a physics bot has spawned.
    /// Idle bots keep the rotation until the server moves them.
    pub async fn look_at(&mut self, target: Vec3) -> Result<(), ActionError> {
        let eye = self.eye_position();
        let (before, after) = if self.has_physics() {
            let Some(movement) = self.movement.as_mut() else { return Ok(()) };
            let before = movement.controls;
            movement.controls.look_at(eye, target);
            ((before.yaw, before.pitch), (movement.controls.yaw, movement.controls.pitch))
        } else if let Some((idle, _)) = self.idle.as_mut() {
            let before = idle.facing(&self.state.player);
            idle.look_at(eye, target);
            (before, idle.facing(&self.state.player))
        } else {
            return Ok(());
        };
        tracing::debug!(?eye, ?target, ?before, ?after, "look at");
        let yaw_delta = ((after.0 - before.0 + 180.0).rem_euclid(360.0) - 180.0).abs();
        if yaw_delta > AIM_TOLERANCE || (after.1 - before.1).abs() > AIM_TOLERANCE {
            self.next_tick(|_, _| false).await?;
        }
        Ok(())
    }
}
