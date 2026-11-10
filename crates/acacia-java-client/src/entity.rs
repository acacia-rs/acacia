//! Other entities as the client tracks them: position as a vanilla client sees it, and the data bots react to.

use acacia_java_proto::entity::PositionPath;
use acacia_java_proto::entity_types::{ENTITY_SHAPES, ENTITY_TYPES, EntityCollision};
use acacia_java_proto::item::ItemStack;
use acacia_java_proto::{Error, Wire, read_varint, read_varlong};

use acacia_java_physics::Aabb;

use crate::interpolation::Lerp;

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub uuid: u128,
    /// `entity_type` registry id.
    pub kind: i32,
    /// Where a vanilla client sees it this tick: interpolated towards the server's position.
    pub position: [f64; 3],
    pub y_rot: f32,
    pub x_rot: f32,
    pub on_ground: bool,
    /// A mob's aggressive flag: set while a melee goal attacks its target (Java names no target).
    pub aggressive: bool,
    /// `Pose` id from entity data (0 standing).
    pub pose: i32,
    /// The type's synced baby flag.
    pub baby: bool,
    /// A happy ghast holding still for its riders, which players can then collide with.
    pub stays_still: bool,
    /// A firework rocket's attached entity, which it boosts while that one glides.
    pub attached_to: Option<i32>,
    /// Riders by entity id, the controlling one first.
    pub passengers: Vec<i32>,
    /// The last `SetEntityMotion` since the last tick, which a boat boarded now starts with.
    pub(crate) motion: [f64; 3],
    /// `VecDeltaCodec` base: the server's last position, which relative moves decode from.
    pub(crate) base: [f64; 3],
    pub(crate) lerp: Lerp,
}

/// Squared distance beyond which sync and teleport packets snap instead of interpolating.
const SNAP_DISTANCE_SQR: f64 = 4096.0;

impl Entity {
    pub fn new(uuid: u128, kind: i32, position: [f64; 3], y_rot: f32, x_rot: f32) -> Self {
        let name = usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("");
        Entity { uuid, kind, position, y_rot, x_rot, on_ground: false, aggressive: false, pose: 0, baby: false, stays_still: false, attached_to: None, passengers: Vec::new(), motion: [0.0; 3], base: position, lerp: Lerp::for_kind(name) }
    }

    /// `getDimensions` and how movers collide with it.
    // TODO: the scale attribute and per-type shapes (slime size, pufferfish puff, shulker peek) aren't applied.
    pub fn dimensions(&self) -> Option<(f32, f32, EntityCollision)> {
        let shape = ENTITY_SHAPES.get(usize::try_from(self.kind).ok()?)?;
        let player = self.is("minecraft:player");
        let (width, height) = match self.pose {
            POSE_SLEEPING => (0.2, 0.2),
            POSE_DYING if player => (0.2, 0.2),
            POSE_FALL_FLYING | POSE_SWIMMING | POSE_SPIN_ATTACK if player => (0.6, 0.6),
            POSE_CROUCHING if player => (0.6, 1.5),
            _ => match shape.baby.filter(|_| self.baby) {
                Some((w, h, _)) => (w, h),
                None => (shape.width, shape.height),
            },
        };
        Some((width, height, shape.collision))
    }

    /// Its box when it blocks `mover` (`canCollideWith`).
    pub fn collision_box(&self, mover: Mover) -> Option<Aabb> {
        let (width, height, collision) = self.dimensions()?;
        let bb = Aabb::of_entity(self.position, width, height);
        let blocks = match (collision, mover) {
            (EntityCollision::Solid, _) | (EntityCollision::Pushable, Mover::Vehicle) => true,
            (_, Mover::Player { feet }) => self.is(HAPPY_GHAST) && !self.baby && (feet >= bb.max[1] || self.stays_still),
            _ => false,
        };
        blocks.then_some(bb)
    }

    fn is(&self, name: &str) -> bool {
        usize::try_from(self.kind).ok().and_then(|k| ENTITY_TYPES.get(k)) == Some(&name)
    }

    /// Pose and baby flag from `SetEntityData` items.
    pub(crate) fn read_body_data(&mut self, body: &[u8]) -> Result<(), Error> {
        let input = &mut &body[..];
        if seek_data(input, POSE)? == Some(SERIALIZER_POSE) {
            self.pose = read_varint(input)?;
        }
        let baby = ENTITY_SHAPES.get(usize::try_from(self.kind).unwrap_or(usize::MAX)).and_then(|s| s.baby);
        if let Some((_, _, index)) = baby {
            self.baby = read_flag(body, index)?.unwrap_or(self.baby);
        }
        if self.is(HAPPY_GHAST) {
            self.stays_still = read_flag(body, HAPPY_GHAST_STAYS_STILL)?.unwrap_or(self.stays_still);
        }
        Ok(())
    }

    /// `moveOrInterpolateTo`; positions farther than vanilla interpolates snap when `may_snap`.
    pub(crate) fn move_to(&mut self, path: Option<PositionPath>, rot: Option<(f32, f32)>, may_snap: bool) {
        let end = path.as_ref().map(PositionPath::end);
        if let Some(end) = end.filter(|e| may_snap && (0..3).map(|i| (e[i] - self.position[i]).powi(2)).sum::<f64>() > SNAP_DISTANCE_SQR) {
            self.position = end;
        } else {
            self.lerp.interpolate_to(&mut self.position, (self.y_rot, self.x_rot), path, rot);
        }
        if let Some((y, x)) = rot {
            (self.y_rot, self.x_rot) = (y, x);
        }
    }
}

/// `Mob.DATA_MOB_FLAGS_ID`: after Entity's 8 data ids and LivingEntity's 7.
pub(crate) const MOB_FLAGS: u8 = 15;
pub(crate) const MOB_AGGRESSIVE: u8 = 4;

pub(crate) fn is_rocket(kind: i32) -> bool {
    usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)) == Some(&"minecraft:firework_rocket")
}

/// Living types whose index 15 is not Mob's flags.
pub(crate) fn has_mob_flags(kind: i32) -> bool {
    let name = usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("");
    !matches!(name, "minecraft:player" | "minecraft:armor_stand" | "minecraft:mannequin")
}

/// `FireworkRocketEntity.DATA_ATTACHED_TO_TARGET`, an optional unsigned int (0 none, else entity id + 1).
pub(crate) const ROCKET_TARGET: u8 = 9;
const SERIALIZER_BYTE: i32 = 0;
const SERIALIZER_BOOLEAN: i32 = 8;
const SERIALIZER_POSE: i32 = 20;
/// `Entity.DATA_POSE`, and the `Pose` ids that change a size.
const POSE: u8 = 6;
const POSE_FALL_FLYING: i32 = 1;
const POSE_SLEEPING: i32 = 2;
const POSE_SWIMMING: i32 = 3;
const POSE_SPIN_ATTACK: i32 = 4;
const POSE_CROUCHING: i32 = 5;
const POSE_DYING: i32 = 7;
const HAPPY_GHAST: &str = "minecraft:happy_ghast";
/// `HappyGhast.STAYS_STILL`: after AgeableMob's 16 and 17 and its leash flag.
const HAPPY_GHAST_STAYS_STILL: u8 = 19;

/// What moves, for [`Entity::collision_box`]: the player (feet height) or a vehicle.
#[derive(Clone, Copy, Debug)]
pub enum Mover {
    Player { feet: f64 },
    Vehicle,
}
const SERIALIZER_OPTIONAL_UNSIGNED_INT: i32 = 19;

/// Mob's flags byte from `SetEntityData` items (updates carry only changed items, so a flag change comes on its own).
pub(crate) fn read_mob_flags(input: &mut &[u8]) -> Result<Option<u8>, Error> {
    match seek_data(input, MOB_FLAGS)? {
        Some(SERIALIZER_BYTE) => Ok(Some(u8::read(input)?)),
        _ => Ok(None),
    }
}

/// The entity a firework rocket's data attaches it to: `Some(None)` when detached, `None` when the item is absent.
pub(crate) fn read_rocket_target(input: &mut &[u8]) -> Result<Option<Option<i32>>, Error> {
    match seek_data(input, ROCKET_TARGET)? {
        Some(SERIALIZER_OPTIONAL_UNSIGNED_INT) => Ok(Some(read_varint(input)?.checked_sub(1).filter(|&id| id >= 0))),
        _ => Ok(None),
    }
}

/// A boolean data item, `None` when absent.
fn read_flag(body: &[u8], index: u8) -> Result<Option<bool>, Error> {
    let input = &mut &body[..];
    match seek_data(input, index)? {
        Some(SERIALIZER_BOOLEAN) => Ok(Some(bool::read(input)?)),
        _ => Ok(None),
    }
}

/// Moves `input` to data item `wanted`'s value, skipping the items before it, and returns its serializer; `None` if
/// absent or behind a value this does not size.
fn seek_data(input: &mut &[u8], wanted: u8) -> Result<Option<i32>, Error> {
    loop {
        let index = u8::read(input)?;
        if index == 0xff || index > wanted {
            return Ok(None);
        }
        let serializer = read_varint(input)?;
        if index == wanted {
            return Ok(Some(serializer));
        }
        // EntityDataSerializers registration order: byte 0, int 1, long 2, float 3, item stack 7, boolean 8,
        // rotations 9, block pos 10, optional block pos 11, direction 12, optional unsigned int 19, pose 20.
        match serializer {
            0 | 8 => drop(u8::read(input)?),
            1 | 12 | 19 | 20 => drop(read_varint(input)?),
            2 => drop(read_varlong(input)?),
            3 => drop(f32::read(input)?),
            7 => drop(Option::<ItemStack>::read(input)?),
            9 => drop((f32::read(input)?, f32::read(input)?, f32::read(input)?)),
            10 => drop(i64::read(input)?),
            11 => {
                if bool::read(input)? {
                    i64::read(input)?;
                }
            }
            _ => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str) -> i32 {
        ENTITY_TYPES.iter().position(|&n| n == name).unwrap() as i32
    }

    #[test]
    fn poses_and_babies_size_the_box() {
        let mut player = Entity::new(0, kind("minecraft:player"), [0.0; 3], 0.0, 0.0);
        player.read_body_data(&[0, 0, 0, POSE, 20, 5, 0xff]).unwrap();
        assert_eq!(player.dimensions(), Some((0.6, 1.5, EntityCollision::Pushable)));
        let mut pig = Entity::new(0, kind("minecraft:pig"), [0.0; 3], 0.0, 0.0);
        pig.read_body_data(&[16, 8, 1, 0xff]).unwrap();
        assert_eq!(pig.dimensions(), Some((0.45, 0.45, EntityCollision::Pushable)));
        let boat = Entity::new(0, kind("minecraft:oak_boat"), [0.0; 3], 0.0, 0.0);
        assert_eq!(boat.dimensions(), Some((1.375, 0.5625, EntityCollision::Solid)));
    }

    #[test]
    fn mob_flags_are_found_past_sized_items_and_not_past_others() {
        // Health (9, float), then the mob flags byte with the aggressive bit.
        let mut data = vec![9, 3];
        20.0_f32.write(&mut data);
        data.extend([MOB_FLAGS, 0, MOB_AGGRESSIVE, 0xff]);
        assert_eq!(read_mob_flags(&mut &data[..]), Ok(Some(MOB_AGGRESSIVE)));
        // A custom name (optional component) cannot be skipped.
        assert_eq!(read_mob_flags(&mut &[2, 6, 1, 0xff][..]), Ok(None));
        assert_eq!(read_mob_flags(&mut &[0, 0, 0, 0xff][..]), Ok(None));
    }
}
