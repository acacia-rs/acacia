//! Riptide on the client: how long the held item has been in use, and the release that launches the player.

use acacia_java_physics::{Aabb, Nearby};
use acacia_java_proto::entity_types::EntityCollision;

use crate::join::Client;

/// `TridentItem.releaseUsing`: shorter holds do nothing.
const MIN_HOLD_TICKS: u32 = 10;

#[derive(Clone, Debug, Default)]
pub(crate) struct Charge {
    /// Ticks the use has run (`getUseDuration - remainingTime`), from the tick its `UseItem` went out.
    held: Option<u32>,
    /// A release queued for the next tick, with the trident's riptide strength when it has the enchantment.
    release: Option<Option<f32>>,
}

impl Client {
    pub(crate) fn start_charge(&mut self) {
        self.charge.held = Some(0);
    }

    pub(crate) fn queue_release(&mut self, riptide: Option<f32>) {
        self.charge.release = Some(riptide);
    }

    /// The release's client side, after the tick's queued actions went out and before the player moves, where
    /// `handleKeybinds` runs it.
    pub(crate) fn apply_release(&mut self) {
        let Some(riptide) = self.charge.release.take() else { return };
        let held = self.charge.held.take().unwrap_or(0);
        let wet = self.movement.player.in_water || self.in_rain();
        let launches = held >= MIN_HOLD_TICKS && wet && self.movement.riding.is_none();
        if let Some(strength) = riptide.filter(|s| *s > 0.0 && launches) {
            self.movement.player.riptide(&self.world, strength);
        }
    }

    /// After the player's tick: the use ran one more tick.
    pub(crate) fn tick_charge(&mut self) {
        if let Some(held) = &mut self.charge.held {
            *held += 1;
        }
    }

    /// Before the player's tick, while a spin runs: the entities it could hit, where they are this tick.
    pub(crate) fn spin_targets(&mut self) {
        let me = self.world.player_id;
        let spinning = self.movement.player.auto_spin_ticks() > 0;
        self.movement.player.nearby = if spinning {
            self.world
                .entities
                .iter()
                .filter(|(id, _)| **id != me)
                .filter_map(|(_, e)| {
                    let (width, height, collision) = e.dimensions()?;
                    let living = collision == EntityCollision::Pushable && !e.type_name().contains("minecart");
                    Some(Nearby { bounds: Aabb::of_entity(e.position, width, height), living })
                })
                .collect()
        } else {
            Vec::new()
        };
    }
}
