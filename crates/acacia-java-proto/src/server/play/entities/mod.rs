//! Entity packets for the server role: spawn and removal, movement, status events, equipment, effects and attributes,
//! the player list, entity metadata ([`data`]) and the serverbound interactions. Every body is a [`Wire`], so both
//! directions round-trip; ids are in `crate::ids::play`. Layouts were checked with `javap -p -c` on the 26.3 packet
//! and codec classes, and the ones azalea had wrong (see the bridge's docs/PROTOCOL_AUDIT.md) follow the vanilla codec.

pub mod data;
pub mod data_types;
pub mod effects;
pub mod movement;
pub mod particle;
pub mod player_info;
pub mod profile;
pub mod serverbound;
pub mod spawn;
pub mod status;
#[cfg(test)]
mod data_tests;
#[cfg(test)]
mod tests;

use crate::{Error, VarInt, Wire};

/// `ByteBufCodecs.optional`: a presence flag, then the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Optional<T>(pub Option<T>);

impl<T> Default for Optional<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: Wire> Wire for Optional<T> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { T::read(input).map(|v| Self(Some(v))) } else { Ok(Self(None)) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.0.is_some().write(out);
        if let Some(v) = &self.0 {
            v.write(out);
        }
    }
}

/// An entity id that may be absent: `id + 1` as a VarInt, 0 for none (`ClientboundDamageEventPacket`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptEntityId(pub Option<i32>);

impl Wire for OptEntityId {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self(match VarInt::read(input)?.0 {
            0 => None,
            n => Some(n.wrapping_sub(1)),
        }))
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.0.map_or(0, |id| id.wrapping_add(1))).write(out);
    }
}

/// `Mth.packDegrees`: an angle as a byte of 1/256 turns.
pub fn pack_degrees(degrees: f32) -> u8 {
    (degrees * 256.0 / 360.0).floor() as i32 as u8
}

/// `Mth.unpackDegrees`.
pub fn unpack_degrees(packed: u8) -> f32 {
    f32::from(packed as i8) * 360.0 / 256.0
}
