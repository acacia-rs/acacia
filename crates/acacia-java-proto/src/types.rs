//! Small shared value types. `BlockPos` is in [`crate::actions`], `GameProfile` in [`crate::server::login`], colours
//! and components in [`crate::text`].

use crate::actions::BlockPos;
use crate::{Error, VarInt, Wire};

/// `GameType`; the wire is its id as a VarInt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: i32) -> Result<Self, Error> {
        match id {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(Error::Invalid("game mode")),
        }
    }

    pub fn name(self) -> &'static str {
        ["survival", "creative", "adventure", "spectator"][self as usize]
    }
}

impl Wire for GameMode {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Self::from_id(VarInt::read(input)?.0)
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(i32::from(self.id())).write(out);
    }
}

/// `GameType.OPTIONAL_STREAM_CODEC`: 0 for none, else id + 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionalGameMode(pub Option<GameMode>);

impl Wire for OptionalGameMode {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => Ok(Self(None)),
            n => GameMode::from_id(n - 1).map(|m| Self(Some(m))),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.0.map_or(0, |m| i32::from(m.id()) + 1)).write(out);
    }
}

/// An RGB colour; the wire form (particle options) is the ARGB int with full alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb(pub u32);

impl Rgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
    }

    pub fn argb(self) -> i32 {
        (0xFF00_0000 | (self.0 & 0xFF_FFFF)) as i32
    }
}

impl Wire for Rgb {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self(i32::read(input)? as u32 & 0xFF_FFFF))
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.argb().write(out);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Wire for Vec3 {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { x: f64::read(input)?, y: f64::read(input)?, z: f64::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.x.write(out);
        self.y.write(out);
        self.z.write(out);
    }
}

/// `ChunkPos.STREAM_CODEC`: x in the low 32 bits of a long, z in the high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl Wire for ChunkPos {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let v = i64::read(input)?;
        Ok(Self { x: v as i32, z: (v >> 32) as i32 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        (i64::from(self.x) & 0xFFFF_FFFF | i64::from(self.z) << 32).write(out);
    }
}

/// `SectionPos.asLong`: x 22 bits, z 22, y 20 (section coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Wire for SectionPos {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let v = i64::read(input)?;
        Ok(Self { x: (v >> 42) as i32, y: (v << 44 >> 44) as i32, z: (v << 22 >> 42) as i32 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        let (x, y, z) = (i64::from(self.x), i64::from(self.y), i64::from(self.z));
        ((x & 0x3F_FFFF) << 42 | (y & 0xF_FFFF) | (z & 0x3F_FFFF) << 20).write(out);
    }
}

/// `SectionPos.sectionRelativePos`: a block's offset inside its section as a short, x 4 bits, z 4, y 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SectionBlockPos {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl SectionBlockPos {
    pub fn of(pos: BlockPos) -> Self {
        let [x, y, z] = pos.0.map(|v| (v & 15) as u8);
        Self { x, y, z }
    }

    pub fn packed(self) -> i16 {
        i16::from(self.x & 15) << 8 | i16::from(self.z & 15) << 4 | i16::from(self.y & 15)
    }
}

impl Wire for SectionBlockPos {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let v = u16::read(input)?;
        Ok(Self { x: (v >> 8 & 15) as u8, z: (v >> 4 & 15) as u8, y: (v & 15) as u8 })
    }

    fn write(&self, out: &mut Vec<u8>) {
        (self.packed() as u16).write(out);
    }
}

/// `Filterable<T>`: the raw value, then an optional text-filtered variant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filterable<T> {
    pub raw: T,
    pub filtered: Option<T>,
}

impl<T: Wire> Wire for Filterable<T> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let raw = T::read(input)?;
        let filtered = if bool::read(input)? { Some(T::read(input)?) } else { None };
        Ok(Self { raw, filtered })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.raw.write(out);
        self.filtered.is_some().write(out);
        if let Some(filtered) = &self.filtered {
            filtered.write(out);
        }
    }
}

/// `GlobalPos.STREAM_CODEC`: the dimension's identifier, then the position.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlobalPos {
    pub dimension: String,
    pub pos: BlockPos,
}

impl Wire for GlobalPos {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { dimension: String::read(input)?, pos: BlockPos::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.dimension.write(out);
        self.pos.write(out);
    }
}

macro_rules! var_int_enum {
    ($(#[$doc:meta])* $name:ident, $what:literal, [$($variant:ident),*]) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub enum $name { #[default] $($variant),* }

        impl $name {
            const ALL: &[Self] = &[$(Self::$variant),*];
        }

        impl Wire for $name {
            fn read(input: &mut &[u8]) -> Result<Self, Error> {
                let id = VarInt::read(input)?.0;
                usize::try_from(id).ok().and_then(|i| Self::ALL.get(i).copied()).ok_or(Error::Invalid($what))
            }

            fn write(&self, out: &mut Vec<u8>) {
                VarInt(*self as i32).write(out);
            }
        }
    };
}

var_int_enum!(
    /// `Heightmap.Types` as sent in chunk data (`STREAM_CODEC`, by id).
    HeightmapKind, "heightmap type",
    [WorldSurfaceWg, WorldSurface, OceanFloorWg, OceanFloor, MotionBlocking, MotionBlockingNoLeaves]
);

var_int_enum!(
    /// `AttributeModifier.Operation`, by id.
    AttributeModifierOperation, "attribute operation",
    [AddValue, AddMultipliedBase, AddMultipliedTotal]
);

var_int_enum!(
    /// `ObjectiveCriteria.RenderType`, written as an enum ordinal.
    ObjectiveRenderType, "objective render type",
    [Integer, Hearts]
);

pub(crate) use var_int_enum;

#[cfg(test)]
mod tests;
