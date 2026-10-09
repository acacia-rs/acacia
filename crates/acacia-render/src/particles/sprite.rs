//! A textured particle and its game tick: Java's `Particle.tick` (gravity, `move` with collision,
//! friction, ground drag) plus the overrides of the kinds drawn here.

use glam::{DVec3, IVec3, Vec3};

use super::collide::{Blocks, sweep};
use super::kind::Kind;
use super::rng::Rng;
use super::sheet::Set;

/// How a sprite meets what is behind it: Java's opaque layer (cut out at alpha 0.1, Bedrock's
/// `particles_alpha`) or its translucent one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Cutout,
    Alpha,
}

/// Which frame of its set a sprite shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    Fixed(u8),
    /// First to last over its life (Java's `setSpriteFromAge`).
    Age,
    Backwards,
}

/// How the quad's size follows the share of life gone, `f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grow {
    Fixed,
    /// Full within the first 32nd of its life.
    Ramp,
    /// `1 - f²/2`.
    Flame,
    /// `1 - f²`.
    Lava,
    /// `1 - (1 - f)²`.
    Portal,
    /// Shrinks this much a tick.
    Shrink(f32),
    /// Bedrock's campfire smoke: `(1 - f)^0.08`.
    Campfire,
}

/// Where a sprite's brightness comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    World,
    Full,
    /// Block light rising to full over its life (Java's `addSmoothBlockEmission`).
    Ramp,
    /// Full block light, the sky's as it is.
    Block,
}

/// What a kind does on top of the plain tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Behaviour {
    Plain,
    /// Leaves smoke, more early in its life.
    Lava,
    /// Crit colours: green and blue fade each tick.
    Crit,
    /// Hangs under a block, then becomes `next`; lava's cools (its colour) as it hangs.
    Hang { next: Kind, cooling: bool },
    /// Falls until it lands, then becomes `land`.
    Fall { land: Kind },
    /// Rain and splash drops: half vanish on landing, all inside a block or liquid.
    Drop,
    /// Gone once out of water.
    Bubble,
    /// Wanders sideways and fades out over its last 60 ticks.
    Campfire,
    /// Stays put.
    Still,
    /// Draws nothing; scatters explosion puffs each tick.
    Seed,
    /// Drawn in from where it started along its velocity, rising one block.
    Portal { start: DVec3 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub set: Set,
    pub frame: Frame,
    pub position: DVec3,
    pub previous: DVec3,
    /// Blocks per tick.
    pub velocity: Vec3,
    /// Added to the velocity each tick (gravity is negative y).
    pub accel: Vec3,
    pub friction: f32,
    /// Collides with blocks; its box is `width` square, from `position` up.
    pub physics: bool,
    pub width: f32,
    /// Java's `speedUpWhenYMotionIsBlocked`: spreads sideways under a ceiling.
    pub speed_up: bool,
    pub on_ground: bool,
    /// Stopped by a collision for good (Java's `stoppedByCollision`).
    pub stopped: bool,
    pub age: u32,
    pub lifetime: u32,
    /// Half the quad's side at birth, in blocks.
    pub size: f32,
    pub grow: Grow,
    pub color: [f32; 4],
    /// Bedrock's tint gradients: the colour at the end of its life.
    pub fade: Option<[f32; 3]>,
    pub light: Light,
    pub blend: Blend,
    pub behaviour: Behaviour,
}

impl Sprite {
    /// Java's `Particle` defaults: friction 0.98, no gravity, a 0.2 box that collides.
    pub fn new(set: Set, position: DVec3, velocity: Vec3, size: f32, lifetime: u32) -> Sprite {
        Sprite {
            set,
            frame: Frame::Fixed(0),
            position,
            previous: position,
            velocity,
            accel: Vec3::ZERO,
            friction: 0.98,
            physics: true,
            width: 0.2,
            speed_up: false,
            on_ground: false,
            stopped: false,
            age: 0,
            lifetime,
            size,
            grow: Grow::Fixed,
            color: [1.0; 4],
            fade: None,
            light: Light::World,
            blend: Blend::Cutout,
            behaviour: Behaviour::Plain,
        }
    }

    /// Share of its life gone, 0 to 1.
    pub fn life(&self) -> f32 {
        (self.age as f32 / self.lifetime.max(1) as f32).min(1.0)
    }

    /// One game tick; false once it is gone. What it turns into or leaves behind goes to `spawn`.
    pub fn tick(&mut self, blocks: &impl Blocks, rng: &mut Rng, spawn: &mut Vec<(Kind, DVec3, Vec3)>) -> bool {
        self.previous = self.position;
        if self.age >= self.lifetime {
            if let Behaviour::Hang { next, .. } = self.behaviour {
                spawn.push((next, self.position, self.velocity));
            }
            return false;
        }
        self.age += 1;
        match self.behaviour {
            Behaviour::Still => return true,
            Behaviour::Seed => {
                let x = self.age as f32 / self.lifetime as f32;
                for _ in 0..6 {
                    let offset = DVec3::new(f64::from(rng.f32() - rng.f32()), f64::from(rng.f32() - rng.f32()), f64::from(rng.f32() - rng.f32())) * 4.0;
                    spawn.push((Kind::Explosion, self.position + offset, Vec3::new(x, 0.0, 0.0)));
                }
                return true;
            }
            Behaviour::Portal { start } => {
                let p = self.life();
                let t = f64::from(1.0 - (-p + p * p * 2.0));
                self.position = start + self.velocity.as_dvec3() * t + DVec3::new(0.0, f64::from(1.0 - p), 0.0);
                return true;
            }
            Behaviour::Campfire => {
                self.velocity.x += rng.f32() / 5000.0 * rng.sign();
                self.velocity.z += rng.f32() / 5000.0 * rng.sign();
                if self.age + 60 >= self.lifetime && self.color[3] > 0.01 {
                    self.color[3] -= 0.015;
                }
            }
            Behaviour::Hang { cooling: true, .. } => {
                self.color = [1.0, 16.0 / (self.age as f32 + 16.0), 4.0 / (self.age as f32 + 8.0), 1.0];
            }
            _ => {}
        }
        self.velocity += self.accel;
        let moved = self.step(blocks);
        if self.speed_up && moved.y == 0.0 {
            self.velocity.x *= 1.1;
            self.velocity.z *= 1.1;
        }
        self.velocity *= self.friction;
        if self.on_ground {
            self.velocity.x *= 0.7;
            self.velocity.z *= 0.7;
        }
        self.after_move(blocks, rng, spawn)
    }

    /// Java's `move`: returns how far it went.
    fn step(&mut self, blocks: &impl Blocks) -> DVec3 {
        if self.stopped {
            return DVec3::ZERO;
        }
        let wanted = self.velocity.as_dvec3();
        let moved = match self.physics {
            true => {
                let half = f64::from(self.width) / 2.0;
                let min = self.position - DVec3::new(half, 0.0, half);
                sweep(blocks, min, min + DVec3::new(half * 2.0, half * 2.0, half * 2.0), wanted)
            }
            false => wanted,
        };
        self.position += moved;
        self.stopped = self.physics && wanted.y.abs() >= 1e-5 && moved.y.abs() < 1e-5;
        self.on_ground = wanted.y != moved.y && wanted.y < 0.0;
        if wanted.x != moved.x {
            self.velocity.x = 0.0;
        }
        if wanted.z != moved.z {
            self.velocity.z = 0.0;
        }
        moved
    }

    fn after_move(&mut self, blocks: &impl Blocks, rng: &mut Rng, spawn: &mut Vec<(Kind, DVec3, Vec3)>) -> bool {
        let cell = self.position.floor().as_ivec3();
        match self.behaviour {
            Behaviour::Lava if rng.f32() > self.life() => spawn.push((Kind::Smoke, self.position, self.velocity)),
            Behaviour::Crit => {
                self.color[1] *= 0.96;
                self.color[2] *= 0.9;
            }
            Behaviour::Hang { .. } => self.velocity *= 0.02,
            Behaviour::Fall { land } if self.on_ground => {
                spawn.push((land, self.position, Vec3::ZERO));
                return false;
            }
            Behaviour::Drop => {
                if self.on_ground && rng.f32() < 0.5 {
                    return false;
                }
                if inside(blocks, cell, self.position.y) {
                    return false;
                }
            }
            Behaviour::Bubble => return [blocks.block(cell), blocks.liquid(cell)].into_iter().flatten().any(|s| s.is_water()),
            Behaviour::Campfire => return self.color[3] > 0.0,
            _ => {}
        }
        true
    }

    /// Half the quad's side now.
    pub fn quad_size(&self) -> f32 {
        let f = self.life();
        let scale = match self.grow {
            Grow::Fixed => 1.0,
            Grow::Ramp => (f * 32.0).clamp(0.0, 1.0),
            Grow::Flame => 1.0 - f * f * 0.5,
            Grow::Lava => 1.0 - f * f,
            Grow::Portal => 1.0 - (1.0 - f) * (1.0 - f),
            Grow::Shrink(per_tick) => return (self.size - per_tick * self.age as f32).max(0.0),
            Grow::Campfire => (1.0 - f).max(0.0).powf(0.08),
        };
        self.size * scale
    }

    /// The colour now, with its gradient applied.
    pub fn tint(&self) -> [f32; 4] {
        let Some(end) = self.fade else { return self.color };
        let f = self.life();
        let [r, g, b, a] = self.color;
        [r + (end[0] - r) * f, g + (end[1] - g) * f, b + (end[2] - b) * f, a]
    }

    /// Index into the set's `count` frames.
    pub fn frame_index(&self, count: usize) -> usize {
        let last = count.saturating_sub(1);
        let by_age = (self.age.min(self.lifetime) as usize * last) / self.lifetime.max(1) as usize;
        match self.frame {
            Frame::Fixed(i) => usize::from(i).min(last),
            Frame::Age => by_age,
            Frame::Backwards => last - by_age,
        }
    }

    pub fn visible(&self) -> bool {
        self.behaviour != Behaviour::Seed
    }
}

/// Inside the solid part or the liquid of the block at `cell` (Java's `WaterDropParticle`).
fn inside(blocks: &impl Blocks, cell: IVec3, y: f64) -> bool {
    let top = |s: &acacia_world::BlockState| s.boxes.iter().map(|b| b.max[1]).fold(s.fluid_height(), f32::max);
    let height = [blocks.block(cell), blocks.liquid(cell)].into_iter().flatten().map(top).fold(0.0, f32::max);
    height > 0.0 && y < f64::from(cell.y) + f64::from(height)
}
