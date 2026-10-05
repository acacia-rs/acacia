//! Worn armour as the simulation sees it.

use acacia_physics::Equipment;

use crate::interact::breaking::enchantment_level;
use crate::state::{Inventory, ItemRegistry};

const DEPTH_STRIDER: i16 = 7;
const SOUL_SPEED: i16 = 36;
const SWIFT_SNEAK: i16 = 37;
const CHEST: usize = 1;
const LEGS: usize = 2;
const FEET: usize = 3;

pub(crate) fn worn(inventory: &Inventory, items: &ItemRegistry) -> Equipment {
    let level = |slot: usize, id| inventory.armor.get(slot).map_or(0, |s| i32::from(enchantment_level(s, id)));
    let wears = |slot: usize, name| {
        inventory.armor.get(slot).is_some_and(|s| !s.is_empty() && items.name(s.network_id) == Some(name))
    };
    Equipment {
        depth_strider: level(FEET, DEPTH_STRIDER),
        soul_speed: level(FEET, SOUL_SPEED),
        swift_sneak: level(LEGS, SWIFT_SNEAK),
        leather_boots: wears(FEET, "minecraft:leather_boots"),
        elytra: wears(CHEST, "minecraft:elytra"),
    }
}
