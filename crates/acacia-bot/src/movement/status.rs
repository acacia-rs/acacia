//! What the server knows about the player that changes the simulation: effects, armor enchantments,
//! the elytra, and the movement attribute (Speed/Slowness live there, not in the effects).

use acacia_client::proto::types::PlayerAttributesItem;
use acacia_physics as physics;

use crate::interact::breaking::enchantment_level;
use crate::state::{effect, Effects, ItemRegistry, ItemStack};

const DEPTH_STRIDER: i16 = 7;
const SOUL_SPEED: i16 = 36;
const SWIFT_SNEAK: i16 = 37;
/// Armor window order: helmet, chestplate, leggings, boots.
const CHEST: usize = 1;
const LEGS: usize = 2;
const FEET: usize = 3;

pub(crate) fn effects(e: &Effects) -> physics::Effects {
    let amplifier = |id| e.get(id).map(|x| x.amplifier);
    physics::Effects {
        jump_boost: amplifier(effect::JUMP_BOOST),
        levitation: amplifier(effect::LEVITATION),
        slow_falling: e.get(effect::SLOW_FALLING).is_some(),
        weaving: e.get(effect::WEAVING).is_some(),
    }
}

pub(crate) fn equipment(armor: &[ItemStack], items: &ItemRegistry) -> physics::Equipment {
    let slot = |i: usize| armor.get(i).filter(|s| !s.is_empty());
    let name = |i| slot(i).and_then(|s| items.name(s.network_id));
    let level = |i, id| slot(i).map_or(0, |s| i32::from(enchantment_level(s, id)));
    physics::Equipment {
        depth_strider: level(FEET, DEPTH_STRIDER),
        soul_speed: level(FEET, SOUL_SPEED),
        swift_sneak: level(LEGS, SWIFT_SNEAK),
        leather_boots: name(FEET) == Some("minecraft:leather_boots"),
        elytra: name(CHEST) == Some("minecraft:elytra"),
    }
}

/// The `minecraft:movement` value without the sprint modifier, which the simulation applies itself.
pub(crate) fn movement_without_sprint(a: &PlayerAttributesItem) -> f32 {
    const SPRINT_MODIFIER: &str = "d208fc00-42aa-4aad-9276-d5446530de43";
    a.modifiers.iter().filter(|m| m.id == SPRINT_MODIFIER).fold(a.current, |v, m| v / (1.0 + m.amount))
}
