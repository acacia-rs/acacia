//! `ParticleTypes.STREAM_CODEC`: a `particle_type` registry id, then that type's options. Which options a type carries
//! was read off `ParticleTypes`' registrations and each option class's `streamCodec` (javap); every other type has none.
//! Colours are ARGB ints, as `ColorParticleOption` and `SpellParticleOption` write them.

use crate::actions::BlockPos;
use crate::item::Components;
use crate::registry::PARTICLE_TYPE;
use crate::types::Vec3;
use crate::{Error, VarInt, Wire};

/// `ItemStackTemplate.STREAM_CODEC`: item, then count, then the patch (a stack has count first).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemTemplate {
    pub item: i32,
    pub count: i32,
    pub components: Components,
}

impl Wire for ItemTemplate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { item: VarInt::read(input)?.0, count: VarInt::read(input)?.0, components: Components::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.item).write(out);
        VarInt(self.count).write(out);
        self.components.write(out);
    }
}

/// `PositionSource.STREAM_CODEC`: a `position_source_type` id (block 0, entity 1), then the source.
#[derive(Debug, Clone, PartialEq)]
pub enum PositionSource {
    Block(BlockPos),
    Entity { id: i32, y_offset: f32 },
}

impl Wire for PositionSource {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => BlockPos::read(input).map(Self::Block),
            1 => Ok(Self::Entity { id: VarInt::read(input)?.0, y_offset: f32::read(input)? }),
            _ => Err(Error::Invalid("position source type")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Block(pos) => {
                VarInt(0).write(out);
                pos.write(out);
            }
            Self::Entity { id, y_offset } => {
                VarInt(1).write(out);
                VarInt(*id).write(out);
                y_offset.write(out);
            }
        }
    }
}

/// The options of a particle type; the variant must match the type, since writing does not check.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ParticleOptions {
    #[default]
    None,
    /// `block`, `block_marker`, `falling_dust`, `dust_pillar`, `block_crumble`: a block state id (VarInt).
    BlockState(i32),
    /// `geyser`, `geyser_plume` (water blocks), `entity_effect`, `flash`, `tinted_leaves` (ARGB): a big-endian int.
    Int(i32),
    /// `geyser_base`, `geyser_poof` (water blocks, burst impulse), `dust` (RGB, scale), `effect`, `instant_effect`
    /// (ARGB, power).
    IntFloat(i32, f32),
    /// `dragon_breath` (power), `sculk_charge` (roll).
    Float(f32),
    DustTransition { from: i32, to: i32, scale: f32 },
    Item(ItemTemplate),
    Vibration { source: PositionSource, arrival_ticks: i32 },
    Trail { target: Vec3, color: i32, duration: i32 },
    /// `shriek`: the delay in ticks (VarInt).
    Shriek(i32),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Particle {
    /// `particle_type` registry id.
    pub kind: i32,
    pub options: ParticleOptions,
}

impl Particle {
    /// A particle by registry name, e.g. `minecraft:heart` or `heart`.
    pub fn of(name: &str, options: ParticleOptions) -> Result<Self, Error> {
        Ok(Self { kind: PARTICLE_TYPE.require(name)?, options })
    }

    fn read_options(name: &str, input: &mut &[u8]) -> Result<ParticleOptions, Error> {
        use ParticleOptions as O;
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        Ok(match name {
            "block" | "block_marker" | "falling_dust" | "dust_pillar" | "block_crumble" => O::BlockState(VarInt::read(input)?.0),
            "geyser" | "geyser_plume" | "entity_effect" | "flash" | "tinted_leaves" => O::Int(i32::read(input)?),
            "geyser_base" | "geyser_poof" | "dust" | "effect" | "instant_effect" => O::IntFloat(i32::read(input)?, f32::read(input)?),
            "dragon_breath" | "sculk_charge" => O::Float(f32::read(input)?),
            "dust_color_transition" => O::DustTransition { from: i32::read(input)?, to: i32::read(input)?, scale: f32::read(input)? },
            "item" => O::Item(ItemTemplate::read(input)?),
            "vibration" => O::Vibration { source: PositionSource::read(input)?, arrival_ticks: VarInt::read(input)?.0 },
            "trail" => O::Trail { target: Vec3::read(input)?, color: i32::read(input)?, duration: VarInt::read(input)?.0 },
            "shriek" => O::Shriek(VarInt::read(input)?.0),
            _ => O::None,
        })
    }
}

impl Wire for Particle {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let kind = VarInt::read(input)?.0;
        let name = PARTICLE_TYPE.name(kind).ok_or(Error::Invalid("particle type"))?;
        Ok(Self { kind, options: Self::read_options(name, input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        use ParticleOptions as O;
        VarInt(self.kind).write(out);
        match &self.options {
            O::None => {}
            O::BlockState(state) | O::Shriek(state) => VarInt(*state).write(out),
            O::Int(v) => v.write(out),
            O::IntFloat(i, f) => {
                i.write(out);
                f.write(out);
            }
            O::Float(f) => f.write(out),
            O::DustTransition { from, to, scale } => {
                from.write(out);
                to.write(out);
                scale.write(out);
            }
            O::Item(item) => item.write(out),
            O::Vibration { source, arrival_ticks } => {
                source.write(out);
                VarInt(*arrival_ticks).write(out);
            }
            O::Trail { target, color, duration } => {
                target.write(out);
                color.write(out);
                VarInt(*duration).write(out);
            }
        }
    }
}
