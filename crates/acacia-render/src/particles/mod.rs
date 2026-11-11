//! Particles, simulated per game tick on the CPU and drawn as camera-facing quads ([`draw`]):
//! block-break chips ([`chips`]) and, by the look's [`Style`], Java's particle engine ported rule
//! for rule ([`java`]) or Bedrock-like sprites ([`sprite`], [`bedrock`], [`ambient`]), spawned by
//! the server or by the blocks around the player.

mod ambient;
mod bedrock;
mod chips;
mod collide;
mod draw;
pub mod java;
mod kind;
mod rng;
pub mod sheet;
mod sprite;

use glam::{DVec3, IVec3, Vec3};

pub use chips::Chip;
pub use collide::Blocks;
pub use draw::Draw;
pub use kind::Kind;
pub use sheet::{Set, Sheet, Style};
pub use sprite::{Blend, Light, Sprite};

use java::{Options, Type};

/// Chips and sprites beyond these are dropped oldest first.
const MAX_CHIPS: usize = 16_384;
const MAX_SPRITES: usize = 16_384;

#[derive(Default)]
pub struct Particles {
    pub chips: Vec<Chip>,
    /// The Bedrock style's particles.
    pub sprites: Vec<Sprite>,
    /// The Java style's.
    pub java: java::Engine,
    style: Style,
    rng: rng::Rng,
    campfires: ambient::Campfires,
    /// Spawned during a tick, made into sprites after it.
    pending: ambient::Out,
}

impl Particles {
    pub fn style(&self) -> Style {
        self.style
    }

    /// Switches whose particles spawn, dropping the other style's.
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
        self.sprites.clear();
        self.java.clear();
    }

    /// The chips of the block at `pos`, textured from `layer`.
    pub fn break_block(&mut self, pos: IVec3, layer: u16) {
        chips::break_block(pos, layer, &mut self.rng, &mut self.chips);
        if self.chips.len() > MAX_CHIPS {
            self.chips.drain(..self.chips.len() - MAX_CHIPS);
        }
    }

    /// `kind` at `at`, made as the style makes it; `velocity` as Java's `addParticle` takes it.
    pub fn spawn(&mut self, kind: Kind, at: DVec3, velocity: Vec3) {
        if self.style == Style::Java {
            java_spawn(&mut self.java, kind, at, velocity.as_dvec3());
            return;
        }
        match kind {
            Kind::CritBurst | Kind::EnchantedHitBurst => self.burst(kind, at),
            _ => self.sprites.extend(bedrock::sprite(kind, at, velocity, &mut self.rng)),
        }
        if self.sprites.len() > MAX_SPRITES {
            self.sprites.drain(..self.sprites.len() - MAX_SPRITES);
        }
    }

    /// `count` of `kind` at random within `spread` blocks of `at` along each axis.
    pub fn scatter(&mut self, kind: Kind, at: DVec3, velocity: Vec3, count: u32, spread: f32) {
        for _ in 0..count {
            let offset = (Vec3::new(self.rng.f32(), self.rng.f32(), self.rng.f32()) * 2.0 - 1.0) * spread;
            self.spawn(kind, at + offset.as_dvec3(), velocity);
        }
    }

    /// A hit's crits as Bedrock's emitters fire them: 48 (enchanted 16) at 10 to 20 blocks a second.
    fn burst(&mut self, kind: Kind, at: DVec3) {
        let single = if kind == Kind::CritBurst { Kind::Crit } else { Kind::EnchantedHit };
        let count = if kind == Kind::EnchantedHitBurst { 16 } else { 48 };
        for _ in 0..count {
            let d = loop {
                let d = Vec3::new(self.rng.f32(), self.rng.f32(), self.rng.f32()) * 2.0 - 1.0;
                if d.length_squared() <= 1.0 {
                    break d;
                }
            };
            let offset = DVec3::new(0.0, f64::from(self.rng.range(-0.9, -0.5)), 0.0);
            let speed = self.rng.range(10.0, 20.0) / 20.0;
            let direction = Vec3::new(d.x * 0.75, d.y, d.z * 0.75).normalize_or(Vec3::Y);
            self.spawn(single, at + offset, direction * speed);
        }
    }

    /// One game tick; `player` is the cell around which the blocks give off particles, if any,
    /// and `camera` where Java's 32-block spawn limit is measured from.
    pub fn tick(&mut self, blocks: &impl Blocks, player: Option<IVec3>, camera: DVec3) {
        self.chips.retain_mut(|c| c.tick(blocks));
        if self.style == Style::Java {
            self.java.camera = camera;
            if let Some(centre) = player {
                self.java.animate_tick(blocks, centre);
            }
            self.java.tick(blocks);
            return;
        }
        let (rng, pending) = (&mut self.rng, &mut self.pending);
        self.sprites.retain_mut(|s| s.tick(blocks, rng, pending));
        if let Some(centre) = player {
            ambient::animate(blocks, centre, &mut self.rng, &mut self.campfires, &mut self.pending);
            self.campfires.tick(blocks, centre, &mut self.rng, &mut self.pending);
        }
        for (kind, at, velocity) in std::mem::take(&mut self.pending) {
            self.spawn(kind, at, velocity);
        }
    }
}

/// A server's particle as Java's level would add it. Crits come from a tracking emitter on the
/// hit entity, taken as a player-sized box centred on `at`.
fn java_spawn(engine: &mut java::Engine, kind: Kind, at: DVec3, v: DVec3) {
    let ty = match kind {
        Kind::CritBurst | Kind::EnchantedHitBurst => {
            let ty = if kind == Kind::CritBurst { Type::Crit } else { Type::EnchantedHit };
            engine.tracking_emitter(Options::simple(ty), at - DVec3::new(0.0, 0.9, 0.0), 0.6, 1.8, 3);
            return;
        }
        Kind::Dust(c) => return engine.add(&Options::dust(c, 1.0), at, v),
        Kind::Note(pitch) => return engine.add(&Options::simple(Type::Note), at, DVec3::new(f64::from(pitch), 0.0, 0.0)),
        Kind::Flame => Type::Flame,
        Kind::SmallFlame => Type::SmallFlame,
        Kind::SoulFlame => Type::SoulFireFlame,
        Kind::Smoke => Type::Smoke,
        Kind::LargeSmoke => Type::LargeSmoke,
        Kind::CampfireSmoke => Type::CampfireCosySmoke,
        Kind::SignalSmoke => Type::CampfireSignalSmoke,
        Kind::Crit => Type::Crit,
        Kind::EnchantedHit => Type::EnchantedHit,
        Kind::Explosion => Type::Explosion,
        Kind::ExplosionEmitter => Type::ExplosionEmitter,
        Kind::Poof => Type::Poof,
        Kind::Heart => Type::Heart,
        Kind::AngryVillager => Type::AngryVillager,
        Kind::HappyVillager => Type::HappyVillager,
        Kind::Splash => Type::Splash,
        Kind::Rain => Type::Rain,
        Kind::Bubble => Type::Bubble,
        Kind::Lava => Type::Lava,
        Kind::Portal => Type::Portal,
        Kind::DrippingWater => Type::DrippingWater,
        Kind::FallingWater => Type::FallingWater,
        Kind::DrippingLava => Type::DrippingLava,
        Kind::FallingLava => Type::FallingLava,
        Kind::LandingLava => Type::LandingLava,
    };
    engine.add(&Options::simple(ty), at, v);
}

#[cfg(test)]
mod tests;
