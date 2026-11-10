//! Other entities as the client tracks them: position as a vanilla client sees it, and the data bots react to.

use acacia_java_proto::entity::PositionPath;
use acacia_java_proto::entity_types::ENTITY_TYPES;
use acacia_java_proto::{Error, Wire, read_varint, read_varlong};

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
    /// `VecDeltaCodec` base: the server's last position, which relative moves decode from.
    pub(crate) base: [f64; 3],
    pub(crate) lerp: Lerp,
}

/// Squared distance beyond which sync and teleport packets snap instead of interpolating.
const SNAP_DISTANCE_SQR: f64 = 4096.0;

impl Entity {
    pub fn new(uuid: u128, kind: i32, position: [f64; 3], y_rot: f32, x_rot: f32) -> Self {
        let name = usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("");
        Entity { uuid, kind, position, y_rot, x_rot, on_ground: false, aggressive: false, base: position, lerp: Lerp::for_kind(name) }
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

/// Living types whose index 15 is not Mob's flags.
pub(crate) fn has_mob_flags(kind: i32) -> bool {
    let name = usize::try_from(kind).ok().and_then(|k| ENTITY_TYPES.get(k)).copied().unwrap_or("");
    !matches!(name, "minecraft:player" | "minecraft:armor_stand" | "minecraft:mannequin")
}

/// Mob's flags byte from `SetEntityData` items, skipping the fixed-size ones before it; `None` if absent or behind a
/// value this does not size (updates carry only changed items, so a flag change comes on its own).
pub(crate) fn read_mob_flags(input: &mut &[u8]) -> Result<Option<u8>, Error> {
    loop {
        let index = u8::read(input)?;
        if index == 0xff || index > MOB_FLAGS {
            return Ok(None);
        }
        let serializer = read_varint(input)?;
        if index == MOB_FLAGS {
            return Ok((serializer == 0).then(|| u8::read(input)).transpose()?);
        }
        // EntityDataSerializers registration order: byte 0, int 1, long 2, float 3, boolean 8, rotations 9, block
        // pos 10, optional block pos 11, direction 12, pose 20.
        match serializer {
            0 | 8 => drop(u8::read(input)?),
            1 | 12 | 20 => drop(read_varint(input)?),
            2 => drop(read_varlong(input)?),
            3 => drop(f32::read(input)?),
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
