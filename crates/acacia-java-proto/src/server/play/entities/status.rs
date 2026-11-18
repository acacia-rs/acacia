//! Entity status packets: events, animations, damage, pickup and equipment.

use super::{OptEntityId, Optional};
use crate::item::ItemStack;
use crate::packets::packet;
use crate::types::Vec3;
use crate::{Error, VarInt, Wire};

// The entity id is a plain big-endian int here, unlike every other entity packet.
packet!(EntityEvent { entity: i32, event: u8 });
// `action`: wake up 0, critical hit 1, magic critical hit 2 (javap constants); swings are `SwingAnimation` now.
packet!(Animate { entity: VarInt, action: u8 });
packet!(HurtAnimation { entity: VarInt, yaw: f32 });
packet!(TakeItemEntity { item: VarInt, collector: VarInt, amount: VarInt });
// `damage_type` is a `damage_type` registry id, which comes from the configuration sync.
packet!(DamageEvent {
    entity: VarInt,
    damage_type: VarInt,
    cause: OptEntityId,
    direct: OptEntityId,
    source_position: Optional<Vec3>,
});

/// `EquipmentSlot`, by ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EquipmentSlot {
    #[default]
    MainHand,
    OffHand,
    Feet,
    Legs,
    Chest,
    Head,
    Body,
    Saddle,
}

impl EquipmentSlot {
    const ALL: [Self; 8] = [Self::MainHand, Self::OffHand, Self::Feet, Self::Legs, Self::Chest, Self::Head, Self::Body, Self::Saddle];

    pub fn from_id(id: u8) -> Result<Self, Error> {
        Self::ALL.get(usize::from(id)).copied().ok_or(Error::Invalid("equipment slot"))
    }
}

/// Bit 7 of a slot byte: another slot follows.
const MORE_SLOTS: u8 = 0x80;

/// `ClientboundSetEquipmentPacket`: the entity, then `(slot byte, optional stack)` pairs, every byte but the last with
/// bit 7 set. Must hold at least one slot, as the reader stops on the first byte without the bit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetEquipment {
    pub entity: VarInt,
    pub slots: Vec<(EquipmentSlot, Option<ItemStack>)>,
}

impl Wire for SetEquipment {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let entity = VarInt::read(input)?;
        let mut slots = Vec::new();
        loop {
            let byte = u8::read(input)?;
            slots.push((EquipmentSlot::from_id(byte & !MORE_SLOTS)?, Option::<ItemStack>::read(input)?));
            if byte & MORE_SLOTS == 0 {
                return Ok(Self { entity, slots });
            }
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.entity.write(out);
        for (i, (slot, stack)) in self.slots.iter().enumerate() {
            let more = if i + 1 < self.slots.len() { MORE_SLOTS } else { 0 };
            (*slot as u8 | more).write(out);
            stack.write(out);
        }
    }
}
