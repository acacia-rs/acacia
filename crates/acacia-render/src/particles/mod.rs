//! Particles, simulated per game tick on the CPU and drawn as camera-facing quads: block-break
//! chips ([`chips`]), and sprites ([`sprite`]) set up as Java ([`java`]) or Bedrock ([`bedrock`])
//! does by the look's [`Style`], spawned by the server or by the blocks around the player
//! ([`ambient`]).

mod ambient;
mod bedrock;
mod chips;
mod collide;
mod java;
mod kind;
mod rng;
pub mod sheet;
mod sprite;

use glam::{DVec3, IVec3, Vec3};

pub use chips::Chip;
pub use collide::Blocks;
pub use kind::Kind;
pub use sheet::{Set, Sheet, Style};
pub use sprite::{Blend, Light, Sprite};

/// Chips and sprites beyond these are dropped oldest first.
const MAX_CHIPS: usize = 16_384;
const MAX_SPRITES: usize = 16_384;

#[derive(Default)]
pub struct Particles {
    pub chips: Vec<Chip>,
    pub sprites: Vec<Sprite>,
    pub style: Style,
    rng: rng::Rng,
    campfires: ambient::Campfires,
    /// Spawned during a tick, made into sprites after it.
    pending: ambient::Out,
}

impl Particles {
    /// The chips of the block at `pos`, textured from `layer`.
    pub fn break_block(&mut self, pos: IVec3, layer: u16) {
        chips::break_block(pos, layer, &mut self.rng, &mut self.chips);
        if self.chips.len() > MAX_CHIPS {
            self.chips.drain(..self.chips.len() - MAX_CHIPS);
        }
    }

    /// `kind` at `at`, made as the style makes it; `velocity` as Java's `addParticle` takes it.
    pub fn spawn(&mut self, kind: Kind, at: DVec3, velocity: Vec3) {
        match kind {
            Kind::CritBurst | Kind::EnchantedHitBurst => self.burst(kind, at),
            _ => {
                let made = match self.style {
                    Style::Java => java::sprite(kind, at, velocity, &mut self.rng),
                    Style::Bedrock => bedrock::sprite(kind, at, velocity, &mut self.rng),
                };
                self.sprites.extend(made);
            }
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

    /// A hit's crits: Java's tracking emitter gives 16 a tick for 3 ticks from inside a unit ball,
    /// here at once; Bedrock's emitters fire 48 (enchanted 16) at 10 to 20 blocks a second.
    fn burst(&mut self, kind: Kind, at: DVec3) {
        let single = if kind == Kind::CritBurst { Kind::Crit } else { Kind::EnchantedHit };
        let count = match (self.style, kind) {
            (Style::Bedrock, Kind::EnchantedHitBurst) => 16,
            _ => 48,
        };
        for _ in 0..count {
            let d = loop {
                let d = Vec3::new(self.rng.f32(), self.rng.f32(), self.rng.f32()) * 2.0 - 1.0;
                if d.length_squared() <= 1.0 {
                    break d;
                }
            };
            match self.style {
                Style::Java => self.spawn(single, at + (d * 0.15).as_dvec3(), Vec3::new(d.x, d.y + 0.2, d.z)),
                Style::Bedrock => {
                    let offset = DVec3::new(0.0, f64::from(self.rng.range(-0.9, -0.5)), 0.0);
                    let speed = self.rng.range(10.0, 20.0) / 20.0;
                    let direction = Vec3::new(d.x * 0.75, d.y, d.z * 0.75).normalize_or(Vec3::Y);
                    self.spawn(single, at + offset, direction * speed);
                }
            }
        }
    }

    /// One game tick; `player` is the cell around which the blocks give off particles, if any.
    pub fn tick(&mut self, blocks: &impl Blocks, player: Option<IVec3>) {
        self.chips.retain_mut(|c| c.tick(blocks));
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

#[cfg(test)]
mod tests;
