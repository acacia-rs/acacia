//! Value types of the entity data serializers that are more than a primitive.

use super::Optional;
use crate::packets::packet;
use crate::text::Component;
use crate::{Error, VarInt, Wire};

/// `Pose`, by ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pose {
    #[default]
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    LongJumping,
    Dying,
    Croaking,
    UsingTongue,
    Sitting,
    Roaring,
    Sniffing,
    Emerging,
    Digging,
    Sliding,
    Shooting,
    Inhaling,
}

impl Pose {
    const ALL: [Self; 18] = [
        Self::Standing,
        Self::FallFlying,
        Self::Sleeping,
        Self::Swimming,
        Self::SpinAttack,
        Self::Crouching,
        Self::LongJumping,
        Self::Dying,
        Self::Croaking,
        Self::UsingTongue,
        Self::Sitting,
        Self::Roaring,
        Self::Sniffing,
        Self::Emerging,
        Self::Digging,
        Self::Sliding,
        Self::Shooting,
        Self::Inhaling,
    ];
}

impl Wire for Pose {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = VarInt::read(input)?.0;
        usize::try_from(id).ok().and_then(|i| Self::ALL.get(i).copied()).ok_or(Error::Invalid("pose"))
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(*self as i32).write(out);
    }
}

// `Rotations.STREAM_CODEC` and `ByteBufCodecs.VECTOR3F`: three floats.
packet!(Rotations { x: f32, y: f32, z: f32 });
// `villager_type` and `villager_profession` registry ids, then the level.
packet!(VillagerData { villager_type: VarInt, profession: VarInt, level: VarInt });

/// The `PAINTING_VARIANT` holder: `id + 1` for a registered variant, 0 then the variant inline.
#[derive(Debug, Clone, PartialEq)]
pub enum PaintingHolder {
    Registered(i32),
    Inline { width: i32, height: i32, asset_id: String, title: Optional<Component>, author: Optional<Component> },
}

impl Default for PaintingHolder {
    fn default() -> Self {
        Self::Registered(0)
    }
}

impl Wire for PaintingHolder {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        match VarInt::read(input)?.0 {
            0 => Ok(Self::Inline {
                width: VarInt::read(input)?.0,
                height: VarInt::read(input)?.0,
                asset_id: String::read(input)?,
                title: Optional::read(input)?,
                author: Optional::read(input)?,
            }),
            n => Ok(Self::Registered(n - 1)),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Registered(id) => VarInt(id + 1).write(out),
            Self::Inline { width, height, asset_id, title, author } => {
                VarInt(0).write(out);
                VarInt(*width).write(out);
                VarInt(*height).write(out);
                asset_id.write(out);
                title.write(out);
                author.write(out);
            }
        }
    }
}

/// The animal and mob variants that are a `holderRegistry` id (VarInt); their ids come from the configuration sync.
/// Serializer id = [`FIRST_VARIANT`] + ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantKind {
    Cat,
    CatSound,
    Cow,
    CowSound,
    Wolf,
    WolfSound,
    Frog,
    Pig,
    PigSound,
    Chicken,
    ChickenSound,
    ZombieNautilus,
}

pub const FIRST_VARIANT: u8 = 21;
pub const VARIANTS: [VariantKind; 12] = [
    VariantKind::Cat,
    VariantKind::CatSound,
    VariantKind::Cow,
    VariantKind::CowSound,
    VariantKind::Wolf,
    VariantKind::WolfSound,
    VariantKind::Frog,
    VariantKind::Pig,
    VariantKind::PigSound,
    VariantKind::Chicken,
    VariantKind::ChickenSound,
    VariantKind::ZombieNautilus,
];
