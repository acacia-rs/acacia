//! What the client tracks about a rideable mob to drive it: saddle and harness, the synced data its ridden movement
//! reads, and its movement attributes.

use acacia_java_proto::entity_types::ENTITY_TYPES;
use acacia_java_proto::item::ItemStack;
use acacia_java_proto::{Error, Wire, ids, read_varint, read_varlong};

use crate::entity::{Entity, seek_data};
use crate::world::World;

use ids::play::clientbound as from;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MountData {
    pub saddled: bool,
    /// The body slot holds something: a happy ghast's harness.
    pub harnessed: bool,
    /// A horse rearing (`AbstractHorse` flag 32).
    pub standing: bool,
    pub dashing: bool,
    /// A camel's `LAST_POSE_CHANGE_TICK`.
    pub last_pose_change: i64,
    /// A boost's length the server just started (`DATA_BOOST_TIME`), until the driven mount takes it.
    pub boost: Option<i32>,
    pub movement_speed: Option<f64>,
    pub jump_strength: Option<f64>,
    pub flying_speed: Option<f64>,
}

const SLOT_BODY: u8 = 6;
const SLOT_SADDLE: u8 = 7;
const ATTRIBUTE_FLYING_SPEED: i32 = 15;
const ATTRIBUTE_JUMP_STRENGTH: i32 = 19;
const ATTRIBUTE_MOVEMENT_SPEED: i32 = 26;
/// The strider's own modifier, which the driven strider applies itself.
const SUFFOCATING: &str = "minecraft:suffocating";
const HORSE_FLAGS: u8 = 18;
const HORSE_STANDING: u8 = 32;
const CAMEL_DASH: u8 = 19;
const CAMEL_POSE_CHANGE: u8 = 20;
const BOOST_TIME: u8 = 18;
const NAUTILUS_DASH: u8 = 20;
const SERIALIZER_BYTE: i32 = 0;
const SERIALIZER_INT: i32 = 1;
const SERIALIZER_LONG: i32 = 2;
const SERIALIZER_BOOLEAN: i32 = 8;

/// A type name without its namespace.
pub(crate) fn short_name(kind: i32) -> &'static str {
    let name = usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("");
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// One data item's value when it has `serializer`.
fn read_item<T>(body: &[u8], index: u8, serializer: i32, read: impl Fn(&mut &[u8]) -> Result<T, Error>) -> Result<Option<T>, Error> {
    let input = &mut &body[..];
    if seek_data(input, index)? == Some(serializer) { read(input).map(Some) } else { Ok(None) }
}

/// The mount items one `SetEntityData` carried.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Synced {
    pub standing: Option<bool>,
    pub dashing: Option<bool>,
    pub last_pose_change: Option<i64>,
    pub boost: Option<i32>,
}

impl Synced {
    pub(crate) fn read(kind: i32, body: &[u8]) -> Result<Synced, Error> {
        let name = short_name(kind);
        let flag = |index| read_item(body, index, SERIALIZER_BOOLEAN, |i| bool::read(i));
        let mut synced = Synced::default();
        if matches!(name, "horse" | "donkey" | "mule" | "skeleton_horse" | "zombie_horse" | "camel" | "camel_husk") {
            synced.standing = read_item(body, HORSE_FLAGS, SERIALIZER_BYTE, |i| u8::read(i))?.map(|f| f & HORSE_STANDING != 0);
        }
        match name {
            "camel" | "camel_husk" => {
                synced.dashing = flag(CAMEL_DASH)?;
                synced.last_pose_change = read_item(body, CAMEL_POSE_CHANGE, SERIALIZER_LONG, read_varlong)?;
            }
            "pig" | "strider" => synced.boost = read_item(body, BOOST_TIME, SERIALIZER_INT, read_varint)?,
            "nautilus" | "zombie_nautilus" => synced.dashing = flag(NAUTILUS_DASH)?,
            _ => {}
        }
        Ok(synced)
    }
}

impl MountData {
    pub(crate) fn apply(&mut self, synced: Synced) {
        self.standing = synced.standing.unwrap_or(self.standing);
        self.dashing = synced.dashing.unwrap_or(self.dashing);
        self.last_pose_change = synced.last_pose_change.unwrap_or(self.last_pose_change);
        self.boost = synced.boost.or(self.boost);
    }

    /// `ClientboundSetEquipmentPacket`'s slots: (slot, more follow) pairs.
    fn read_equipment(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        loop {
            let slot = u8::read(input)?;
            let filled = Option::<ItemStack>::read(input)?.is_some();
            match slot & 0x7f {
                SLOT_SADDLE => self.saddled = filled,
                SLOT_BODY => self.harnessed = filled,
                _ => {}
            }
            if slot & 0x80 == 0 {
                return Ok(());
            }
        }
    }

    /// `ClientboundUpdateAttributesPacket`: each attribute's value from its base and modifiers.
    fn read_attributes(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        for _ in 0..read_varint(input)? {
            let attribute = read_varint(input)?;
            let base = f64::read(input)?;
            let (mut added, mut multiplied_base, mut total) = (0.0, 0.0, 1.0);
            for _ in 0..read_varint(input)? {
                let id = String::read(input)?;
                let amount = f64::read(input)?;
                match read_varint(input)? {
                    _ if id == SUFFOCATING => {}
                    0 => added += amount,
                    1 => multiplied_base += amount,
                    _ => total *= 1.0 + amount,
                }
            }
            let raised = base + added;
            let value = Some((raised + raised * multiplied_base) * total);
            match attribute {
                ATTRIBUTE_MOVEMENT_SPEED => self.movement_speed = value,
                ATTRIBUTE_JUMP_STRENGTH => self.jump_strength = value,
                ATTRIBUTE_FLYING_SPEED => self.flying_speed = value,
                _ => {}
            }
        }
        Ok(())
    }
}

impl World {
    /// Equipment and attributes of other entities; the player's own attributes are read in `Movement`.
    pub(crate) fn apply_mount_packet(&mut self, id: i32, input: &mut &[u8]) -> Result<(), Error> {
        let entity = read_varint(input)?;
        let Some(Entity { mount, .. }) = self.entities.get_mut(&entity) else { return Ok(()) };
        match id {
            from::SET_EQUIPMENT => mount.read_equipment(input),
            from::UPDATE_ATTRIBUTES => mount.read_attributes(input),
            _ => Ok(()),
        }
    }
}
