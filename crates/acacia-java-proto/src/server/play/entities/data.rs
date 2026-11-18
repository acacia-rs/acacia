//! `SynchedEntityData`: the `EntityDataSerializers` set and the packed value list `SetEntityData` carries. The ids are
//! the serializers' registration order (javap of `EntityDataSerializers.<clinit>`); each value's layout is that
//! serializer's `StreamCodec`. Which index holds what is per entity class and lives with the caller.

use super::data_types::{FIRST_VARIANT, PaintingHolder, Pose, Rotations, VARIANTS, VariantKind, VillagerData};
use super::particle::Particle;
use super::profile::ResolvableProfile;
use crate::actions::{BlockPos, Direction};
use crate::item::ItemStack;
use crate::packets::packet;
use crate::text::Component;
use crate::types::GlobalPos;
use crate::{Error, VarInt, Wire, read_varlong};

/// Ends the item list where an index byte would be.
pub const END: u8 = 0xFF;

/// One entity data value; the variant fixes the serializer.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    Byte(u8),
    Int(i32),
    Long(i64),
    Float(f32),
    String(String),
    Component(Component),
    OptionalComponent(Option<Component>),
    ItemStack(Option<ItemStack>),
    Boolean(bool),
    Rotations(Rotations),
    BlockPos(BlockPos),
    OptionalBlockPos(Option<BlockPos>),
    Direction(Direction),
    /// An entity reference: its UUID.
    OptionalLivingEntityReference(Option<u128>),
    /// A block state id.
    BlockState(i32),
    /// 0 on the wire for none.
    OptionalBlockState(Option<i32>),
    Particle(Particle),
    Particles(Vec<Particle>),
    VillagerData(VillagerData),
    /// `value + 1` on the wire, 0 for none.
    OptionalUnsignedInt(Option<u32>),
    Pose(Pose),
    Variant(VariantKind, i32),
    OptionalGlobalPos(Option<GlobalPos>),
    PaintingVariant(PaintingHolder),
    SnifferState(i32),
    ArmadilloState(i32),
    CopperGolemState(i32),
    WeatheringCopperState(i32),
    Vector3(Rotations),
    /// x, y, z, w.
    Quaternion([f32; 4]),
    ResolvableProfile(ResolvableProfile),
    /// `HumanoidArm` ordinal: left 0, right 1.
    HumanoidArm(i32),
    /// `DyeColor` id, white 0 to black 15.
    DyeColor(i32),
}

fn write_optional<T: Wire>(value: &Option<T>, out: &mut Vec<u8>) {
    value.is_some().write(out);
    if let Some(v) = value {
        v.write(out);
    }
}

fn read_optional<T: Wire>(input: &mut &[u8]) -> Result<Option<T>, Error> {
    if bool::read(input)? { T::read(input).map(Some) } else { Ok(None) }
}

/// `VarLong.write`.
fn write_varlong(value: i64, out: &mut Vec<u8>) {
    let mut v = value as u64;
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

impl DataValue {
    /// The `EntityDataSerializers` id.
    pub fn serializer_id(&self) -> u8 {
        match self {
            Self::Byte(_) => 0,
            Self::Int(_) => 1,
            Self::Long(_) => 2,
            Self::Float(_) => 3,
            Self::String(_) => 4,
            Self::Component(_) => 5,
            Self::OptionalComponent(_) => 6,
            Self::ItemStack(_) => 7,
            Self::Boolean(_) => 8,
            Self::Rotations(_) => 9,
            Self::BlockPos(_) => 10,
            Self::OptionalBlockPos(_) => 11,
            Self::Direction(_) => 12,
            Self::OptionalLivingEntityReference(_) => 13,
            Self::BlockState(_) => 14,
            Self::OptionalBlockState(_) => 15,
            Self::Particle(_) => 16,
            Self::Particles(_) => 17,
            Self::VillagerData(_) => 18,
            Self::OptionalUnsignedInt(_) => 19,
            Self::Pose(_) => 20,
            Self::Variant(kind, _) => FIRST_VARIANT + VARIANTS.iter().position(|k| k == kind).expect("listed kind") as u8,
            Self::OptionalGlobalPos(_) => 33,
            Self::PaintingVariant(_) => 34,
            Self::SnifferState(_) => 35,
            Self::ArmadilloState(_) => 36,
            Self::CopperGolemState(_) => 37,
            Self::WeatheringCopperState(_) => 38,
            Self::Vector3(_) => 39,
            Self::Quaternion(_) => 40,
            Self::ResolvableProfile(_) => 41,
            Self::HumanoidArm(_) => 42,
            Self::DyeColor(_) => 43,
        }
    }

    fn read(serializer: u8, input: &mut &[u8]) -> Result<Self, Error> {
        Ok(match serializer {
            0 => Self::Byte(u8::read(input)?),
            1 => Self::Int(VarInt::read(input)?.0),
            2 => Self::Long(read_varlong(input)?),
            3 => Self::Float(f32::read(input)?),
            4 => Self::String(String::read(input)?),
            5 => Self::Component(Component::read(input)?),
            6 => Self::OptionalComponent(read_optional(input)?),
            7 => Self::ItemStack(Option::<ItemStack>::read(input)?),
            8 => Self::Boolean(bool::read(input)?),
            9 => Self::Rotations(Rotations::read(input)?),
            10 => Self::BlockPos(BlockPos::read(input)?),
            11 => Self::OptionalBlockPos(read_optional(input)?),
            12 => Self::Direction(Direction::from_index(VarInt::read(input)?.0)?),
            13 => Self::OptionalLivingEntityReference(read_optional(input)?),
            14 => Self::BlockState(VarInt::read(input)?.0),
            15 => Self::OptionalBlockState(match VarInt::read(input)?.0 {
                0 => None,
                n => Some(n),
            }),
            16 => Self::Particle(Particle::read(input)?),
            17 => Self::Particles(Vec::read(input)?),
            18 => Self::VillagerData(VillagerData::read(input)?),
            19 => Self::OptionalUnsignedInt(match VarInt::read(input)?.0 {
                0 => None,
                n => Some((n - 1) as u32),
            }),
            20 => Self::Pose(Pose::read(input)?),
            21..=32 => Self::Variant(VARIANTS[usize::from(serializer - FIRST_VARIANT)], VarInt::read(input)?.0),
            33 => Self::OptionalGlobalPos(read_optional(input)?),
            34 => Self::PaintingVariant(PaintingHolder::read(input)?),
            35 => Self::SnifferState(VarInt::read(input)?.0),
            36 => Self::ArmadilloState(VarInt::read(input)?.0),
            37 => Self::CopperGolemState(VarInt::read(input)?.0),
            38 => Self::WeatheringCopperState(VarInt::read(input)?.0),
            39 => Self::Vector3(Rotations::read(input)?),
            40 => Self::Quaternion([f32::read(input)?, f32::read(input)?, f32::read(input)?, f32::read(input)?]),
            41 => Self::ResolvableProfile(ResolvableProfile::read(input)?),
            42 => Self::HumanoidArm(VarInt::read(input)?.0),
            43 => Self::DyeColor(VarInt::read(input)?.0),
            _ => return Err(Error::Invalid("entity data serializer")),
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Byte(v) => v.write(out),
            Self::Int(v) | Self::BlockState(v) | Self::SnifferState(v) | Self::ArmadilloState(v) => VarInt(*v).write(out),
            Self::CopperGolemState(v) | Self::WeatheringCopperState(v) | Self::HumanoidArm(v) | Self::DyeColor(v) | Self::Variant(_, v) => {
                VarInt(*v).write(out)
            }
            Self::Long(v) => write_varlong(*v, out),
            Self::Float(v) => v.write(out),
            Self::String(v) => v.write(out),
            Self::Component(v) => v.write(out),
            Self::OptionalComponent(v) => write_optional(v, out),
            Self::ItemStack(v) => v.write(out),
            Self::Boolean(v) => v.write(out),
            Self::Rotations(v) | Self::Vector3(v) => v.write(out),
            Self::BlockPos(v) => v.write(out),
            Self::OptionalBlockPos(v) => write_optional(v, out),
            Self::Direction(v) => VarInt(*v as i32).write(out),
            Self::OptionalLivingEntityReference(v) => write_optional(v, out),
            Self::OptionalBlockState(v) => VarInt(v.unwrap_or(0)).write(out),
            Self::Particle(v) => v.write(out),
            Self::Particles(v) => v.write(out),
            Self::VillagerData(v) => v.write(out),
            Self::OptionalUnsignedInt(v) => VarInt(v.map_or(0, |n| n as i32 + 1)).write(out),
            Self::Pose(v) => v.write(out),
            Self::OptionalGlobalPos(v) => write_optional(v, out),
            Self::PaintingVariant(v) => v.write(out),
            Self::Quaternion(v) => v.iter().for_each(|c| c.write(out)),
            Self::ResolvableProfile(v) => v.write(out),
        }
    }
}

/// One `DataValue` with its index (`SynchedEntityData.DataValue`).
#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    pub index: u8,
    pub value: DataValue,
}

impl DataItem {
    pub fn new(index: u8, value: DataValue) -> Self {
        Self { index, value }
    }
}

/// The item list of `SetEntityData`: `(index, serializer id, value)` items then [`END`]. An index of 0xFF is the
/// terminator, so it cannot be used.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityData(pub Vec<DataItem>);

impl Wire for EntityData {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let mut items = Vec::new();
        loop {
            let index = u8::read(input)?;
            if index == END {
                return Ok(Self(items));
            }
            let serializer = u8::try_from(VarInt::read(input)?.0).map_err(|_| Error::Invalid("entity data serializer"))?;
            items.push(DataItem { index, value: DataValue::read(serializer, input)? });
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        for item in &self.0 {
            debug_assert_ne!(item.index, END, "0xFF ends the list");
            item.index.write(out);
            VarInt(i32::from(item.value.serializer_id())).write(out);
            item.value.write(out);
        }
        END.write(out);
    }
}

// `ClientboundSetEntityDataPacket`: the entity id, then the packed items.
packet!(SetEntityData { entity: VarInt, items: EntityData });
