//! What a Java particle class does in its `tick` beyond `Particle.tick`, and the options a
//! particle is spawned with.

use glam::DVec3;

use super::level::Fluid;
use super::types::Type;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DripStage {
    Hang,
    /// Lava's: its colour cools as it hangs.
    CoolingHang,
    Fall,
    FallAndLand,
    Land,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Behaviour {
    Base,
    /// The base tick, then the sprite from its age.
    Animated,
    Lava,
    /// `DripParticle`; `sound` is whether landing plays one (it draws its volume).
    Drip { fluid: Fluid, stage: DripStage, next: Option<Type>, sound: bool },
    WaterDrop,
    Bubble,
    /// `SulfurBubbleParticle`: rises three blocks through still water, growing to 0.15.
    SulfurBubble { y_start: f64, y_end: f64, size_start: f32, y_prev: f64 },
    BubbleColumnUp,
    CurrentDown { angle: f32 },
    SuspendedTown,
    Portal { start: DVec3 },
    ReversePortal,
    /// `FlyTowardsPositionParticle`; `alpha` is its `LifetimeAlpha` (start, end, from, to).
    FlyTowards { start: DVec3, alpha: [f32; 4] },
    Crit,
    HugeExplosion,
    ExplosionSeed,
    Campfire,
    /// `SimpleAnimatedParticle`: fades over the second half of its life towards `fade`.
    SimpleAnimated { fade: Option<[f32; 3]> },
    Leaves { rot_speed: f32, spin: f32, wind: f32, swirl: bool, flow: bool, flow_scale: [f64; 2], swirl_period: f64 },
    FallingDust { rot_speed: f32 },
    /// `DustColorTransitionParticle`: from one colour to the other over its life.
    Transition { from: [f32; 3], to: [f32; 3] },
}

/// The `ParticleOptions` parameters of the option classes used here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub ty: Type,
    /// Dust, tinted leaves and falling dust colour (RGB, alpha).
    pub color: [f32; 4],
    /// `DustParticleOptions.scale`.
    pub scale: f32,
    /// `DustColorTransitionOptions.toColor`.
    pub to_color: [f32; 3],
}

impl Options {
    pub fn simple(ty: Type) -> Options {
        Options { ty, color: [1.0; 4], scale: 1.0, to_color: [1.0, 0.0, 0.0] }
    }

    /// `DustParticleOptions(color, scale)`: the colour as its packed RGB int stores it.
    pub fn dust(rgb: [f32; 3], scale: f32) -> Options {
        let packed = |c: f32| f32::from((c.clamp(0.0, 1.0) * 255.0) as u8) / 255.0;
        Options { color: [packed(rgb[0]), packed(rgb[1]), packed(rgb[2]), 1.0], scale: scale.clamp(0.01, 4.0), ..Options::simple(Type::Dust) }
    }

    /// `DustColorTransitionOptions`.
    pub fn transition(from: [f32; 3], to: [f32; 3], scale: f32) -> Options {
        Options { color: [from[0], from[1], from[2], 1.0], to_color: to, scale: scale.clamp(0.01, 4.0), ..Options::simple(Type::DustColorTransition) }
    }

    /// `ColorParticleOption` (tinted leaves) or `BlockParticleOption`'s dust colour, from `0xRRGGBB`.
    pub fn colored(ty: Type, rgb: u32) -> Options {
        let c = |shift: u32| (rgb >> shift & 255) as f32 / 255.0;
        Options { color: [c(16), c(8), c(0), 1.0], ..Options::simple(ty) }
    }
}

/// `level.addParticle` asked for by a block or a particle: options, position, velocity.
pub type Request = (Options, DVec3, DVec3);
