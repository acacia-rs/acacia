//! `LevelParticles` and the particle options (`ParticleTypes.STREAM_CODEC`): the type's registry id, then options by
//! type. Layouts from the 26.3 option classes; azalea's variants are unit for several of these.

use crate::item::Components;
use crate::actions::BlockPos;
use crate::registry::PARTICLE_TYPE;
use crate::types::Vec3;
use crate::{Error, VarInt, Wire};

/// `ItemStackTemplate.STREAM_CODEC`: item, then count, then the patch. A stack writes count first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemTemplate {
    pub item: VarInt,
    pub count: VarInt,
    pub patch: Components,
}

impl Wire for ItemTemplate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { item: VarInt::read(input)?, count: VarInt::read(input)?, patch: Components::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.item.write(out);
        self.count.write(out);
        self.patch.write(out);
    }
}

/// `PositionSource.STREAM_CODEC`: a type id (block 0, entity 1), then its data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionSource {
    Block(BlockPos),
    Entity { id: i32, y_offset: f32 },
}

impl Wire for PositionSource {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => Ok(Self::Block(BlockPos::read(input)?)),
            1 => Ok(Self::Entity { id: VarInt::read(input)?.0, y_offset: f32::read(input)? }),
            _ => Err(Error::Invalid("position source")),
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

/// The options a particle type carries. Colours are ARGB ints except where noted.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Options {
    #[default]
    None,
    /// `block`, `block_marker`, `falling_dust`, `dust_pillar`, `block_crumble`: a block state id.
    BlockState(i32),
    /// `dust`: an RGB int (no alpha) and a scale.
    Dust { rgb: i32, scale: f32 },
    /// `dust_color_transition`: two RGB ints and a scale.
    DustTransition { from: i32, to: i32, scale: f32 },
    /// `entity_effect`, `tinted_leaves`, `flash`.
    Color(i32),
    /// `effect`, `instant_effect`.
    Spell { argb: i32, power: f32 },
    /// `dragon_breath`.
    Power(f32),
    /// `sculk_charge`: the roll.
    SculkCharge(f32),
    /// `shriek`: the delay in ticks.
    Shriek(i32),
    Item(ItemTemplate),
    Vibration { source: PositionSource, arrival_ticks: i32 },
    /// `trail`: the target, an RGB int and the duration in ticks.
    Trail { target: Vec3, rgb: i32, duration: i32 },
    /// `geyser`, `geyser_plume`.
    Geyser(i32),
    /// `geyser_base`, `geyser_poof`.
    GeyserBase { int: i32, float: f32 },
}

/// Which [`Options`] shape a particle type name takes; types not listed have none.
fn shape(name: &str) -> Options {
    use Options::*;
    match name.strip_prefix("minecraft:").unwrap_or(name) {
        "block" | "block_marker" | "falling_dust" | "dust_pillar" | "block_crumble" => BlockState(0),
        "dust" => Dust { rgb: 0, scale: 0.0 },
        "dust_color_transition" => DustTransition { from: 0, to: 0, scale: 0.0 },
        "entity_effect" | "tinted_leaves" | "flash" => Color(0),
        "effect" | "instant_effect" => Spell { argb: 0, power: 0.0 },
        "dragon_breath" => Power(0.0),
        "sculk_charge" => SculkCharge(0.0),
        "shriek" => Shriek(0),
        "item" => Item(ItemTemplate::default()),
        "vibration" => Vibration { source: PositionSource::Block(BlockPos::default()), arrival_ticks: 0 },
        "trail" => Trail { target: Vec3::default(), rgb: 0, duration: 0 },
        "geyser" | "geyser_plume" => Geyser(0),
        "geyser_base" | "geyser_poof" => GeyserBase { int: 0, float: 0.0 },
        _ => None,
    }
}

impl Options {
    fn same_shape(&self, other: &Options) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn read(like: &Options, input: &mut &[u8]) -> Result<Self, Error> {
        use Options::*;
        Ok(match like {
            None => None,
            BlockState(_) => BlockState(VarInt::read(input)?.0),
            Dust { .. } => Dust { rgb: i32::read(input)?, scale: f32::read(input)? },
            DustTransition { .. } => DustTransition { from: i32::read(input)?, to: i32::read(input)?, scale: f32::read(input)? },
            Color(_) => Color(i32::read(input)?),
            Spell { .. } => Spell { argb: i32::read(input)?, power: f32::read(input)? },
            Power(_) => Power(f32::read(input)?),
            SculkCharge(_) => SculkCharge(f32::read(input)?),
            Shriek(_) => Shriek(VarInt::read(input)?.0),
            Item(_) => Item(ItemTemplate::read(input)?),
            Vibration { .. } => Vibration { source: PositionSource::read(input)?, arrival_ticks: VarInt::read(input)?.0 },
            Trail { .. } => Trail { target: Vec3::read(input)?, rgb: i32::read(input)?, duration: VarInt::read(input)?.0 },
            Geyser(_) => Geyser(i32::read(input)?),
            GeyserBase { .. } => GeyserBase { int: i32::read(input)?, float: f32::read(input)? },
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Options::None => {}
            Options::BlockState(state) => VarInt(*state).write(out),
            Options::Dust { rgb, scale } => {
                rgb.write(out);
                scale.write(out);
            }
            Options::DustTransition { from, to, scale } => {
                from.write(out);
                to.write(out);
                scale.write(out);
            }
            Options::Color(argb) => argb.write(out),
            Options::Spell { argb, power } => {
                argb.write(out);
                power.write(out);
            }
            Options::Power(v) | Options::SculkCharge(v) => v.write(out),
            Options::Shriek(delay) => VarInt(*delay).write(out),
            Options::Item(item) => item.write(out),
            Options::Vibration { source, arrival_ticks } => {
                source.write(out);
                VarInt(*arrival_ticks).write(out);
            }
            Options::Trail { target, rgb, duration } => {
                target.write(out);
                rgb.write(out);
                VarInt(*duration).write(out);
            }
            Options::Geyser(v) => v.write(out),
            Options::GeyserBase { int, float } => {
                int.write(out);
                float.write(out);
            }
        }
    }
}

/// A particle: a `particle_type` registry id and its options.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Particle {
    pub kind: VarInt,
    pub options: Options,
}

impl Particle {
    /// `name` such as `minecraft:heart` or `dust`; `options` must be the shape that type takes (see [`Options`]).
    pub fn named(name: &str, options: Options) -> Result<Self, Error> {
        let kind = PARTICLE_TYPE.id_lenient(name).ok_or(Error::Invalid("particle type"))?;
        if !shape(name).same_shape(&options) {
            return Err(Error::Invalid("particle options"));
        }
        Ok(Self { kind: VarInt(kind), options })
    }
}

impl Wire for Particle {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let kind = VarInt::read(input)?;
        let name = PARTICLE_TYPE.name(kind.0).ok_or(Error::Invalid("particle type"))?;
        Ok(Self { kind, options: Options::read(&shape(name), input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        debug_assert!(
            PARTICLE_TYPE.name(self.kind.0).is_some_and(|n| shape(n).same_shape(&self.options)),
            "options do not fit particle {}",
            self.kind.0
        );
        self.kind.write(out);
        self.options.write(out);
    }
}

/// `ClientboundLevelParticlesPacket.RandomizationType` ids.
pub const RANDOMIZATION_DEFAULT: i32 = 0;

/// vanilla 26.3's layout: the particle first, three max speeds, a VarInt count and a randomization type.
/// azalea's is the older one (particle last, one max speed).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelParticles {
    pub particle: Particle,
    pub override_limiter: bool,
    pub always_show: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub x_dist: f32,
    pub y_dist: f32,
    pub z_dist: f32,
    pub x_max_speed: f32,
    pub y_max_speed: f32,
    pub z_max_speed: f32,
    pub count: VarInt,
    pub randomization: VarInt,
}

impl Wire for LevelParticles {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            particle: Particle::read(input)?,
            override_limiter: bool::read(input)?,
            always_show: bool::read(input)?,
            x: f64::read(input)?,
            y: f64::read(input)?,
            z: f64::read(input)?,
            x_dist: f32::read(input)?,
            y_dist: f32::read(input)?,
            z_dist: f32::read(input)?,
            x_max_speed: f32::read(input)?,
            y_max_speed: f32::read(input)?,
            z_max_speed: f32::read(input)?,
            count: VarInt::read(input)?,
            randomization: VarInt::read(input)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.particle.write(out);
        self.override_limiter.write(out);
        self.always_show.write(out);
        [self.x, self.y, self.z].iter().for_each(|v| v.write(out));
        [self.x_dist, self.y_dist, self.z_dist, self.x_max_speed, self.y_max_speed, self.z_max_speed].iter().for_each(|v| v.write(out));
        self.count.write(out);
        self.randomization.write(out);
    }
}
