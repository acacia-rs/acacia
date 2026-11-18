//! Mob effects and attributes.

use crate::packets::packet;
use crate::types::AttributeModifierOperation;
use crate::wire::read_len;
use crate::{Error, VarInt, Wire};

/// `UpdateMobEffect` flags: ambient, visible particles, show icon, blend.
pub const EFFECT_AMBIENT: u8 = 1;
pub const EFFECT_VISIBLE: u8 = 2;
pub const EFFECT_SHOW_ICON: u8 = 4;
pub const EFFECT_BLEND: u8 = 8;

// `effect` is a `mob_effect` registry id (`crate::registry::MOB_EFFECT`).
packet!(UpdateMobEffect { entity: VarInt, effect: VarInt, amplifier: VarInt, duration: VarInt, flags: u8 });
packet!(RemoveMobEffect { entity: VarInt, effect: VarInt });

/// Vanilla's cap on the snapshots in one `UpdateAttributes`; a longer list fails the client's decode.
pub const MAX_ATTRIBUTES: usize = 128;

// `id` is an identifier such as `minecraft:effect.speed`.
packet!(AttributeModifier { id: String, amount: f64, operation: AttributeModifierOperation });
// `attribute` is an `attribute` registry id (`crate::registry::ATTRIBUTE`).
packet!(AttributeSnapshot { attribute: VarInt, base: f64, modifiers: Vec<AttributeModifier> });

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateAttributes {
    pub entity: VarInt,
    pub attributes: Vec<AttributeSnapshot>,
}

impl Wire for UpdateAttributes {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let entity = VarInt::read(input)?;
        let mut peek = *input;
        if read_len(&mut peek)? > MAX_ATTRIBUTES {
            return Err(Error::Invalid("attribute count"));
        }
        Ok(Self { entity, attributes: Vec::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.entity.write(out);
        self.attributes.write(out);
    }
}
