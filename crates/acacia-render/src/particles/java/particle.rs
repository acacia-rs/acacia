//! One Java particle: `Particle` and `SingleQuadParticle` state, their constructors, `move` and
//! the base `tick`.

use glam::DVec3;

use super::behaviour::Behaviour;
use super::level;
use super::JavaRandom;
use super::types::Type;
use crate::particles::collide::Blocks;

/// How `getQuadSize(a)` follows the share of life gone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Fixed,
    /// `quadSize * clamp((age + a) / lifetime * 32, 0, 1)`.
    GrowIn,
    Flame,
    Lava,
    Portal,
    ReversePortal,
}

/// `getLightCoords`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glow {
    World,
    FullBright,
    /// `LightCoordsUtil.withBlock(light, 15)`.
    Block15,
    /// `addSmoothBlockEmission` with the age fraction.
    AgeEmission,
    /// The same with the age fraction to the fourth power.
    AgeEmission4,
}

#[derive(Debug, Clone)]
pub struct Particle {
    pub ty: Type,
    pub position: DVec3,
    pub previous: DVec3,
    pub velocity: DVec3,
    pub bb_min: DVec3,
    pub bb_max: DVec3,
    pub bb_width: f32,
    pub bb_height: f32,
    pub on_ground: bool,
    pub has_physics: bool,
    /// `move` overridden to shift the box without collision.
    pub free_move: bool,
    pub stopped_by_collision: bool,
    pub removed: bool,
    pub age: i32,
    pub lifetime: i32,
    pub gravity: f32,
    pub friction: f32,
    pub speed_up_when_y_blocked: bool,
    /// `quadSize`.
    pub size: f32,
    pub rgba: [f32; 4],
    pub roll: f32,
    pub o_roll: f32,
    /// The sprite shown, a `textures/particle` name; `None` for particles that draw nothing.
    pub sprite: Option<&'static str>,
    /// Steps through its set by age (`setSpriteFromAge`).
    pub animated: bool,
    pub size_curve: Size,
    pub glow: Glow,
    pub behaviour: Behaviour,
    pub no_render: bool,
    /// Ticked once by its constructor (`CritParticle`).
    pub pre_tick: bool,
    /// `Particle.random`, the particle's own generator.
    pub random: JavaRandom,
    /// What `random` was seeded with, which names the particle to a probe.
    pub seed: i64,
}

impl Particle {
    fn blank(ty: Type, p: DVec3) -> Particle {
        let mut particle = Particle {
            ty,
            position: p,
            previous: p,
            velocity: DVec3::ZERO,
            bb_min: p,
            bb_max: p,
            bb_width: 0.2,
            bb_height: 0.2,
            on_ground: false,
            has_physics: true,
            free_move: false,
            stopped_by_collision: false,
            removed: false,
            age: 0,
            lifetime: 0,
            gravity: 0.0,
            friction: 0.98,
            speed_up_when_y_blocked: false,
            size: 0.0,
            rgba: [1.0; 4],
            roll: 0.0,
            o_roll: 0.0,
            sprite: None,
            animated: false,
            size_curve: Size::Fixed,
            glow: Glow::World,
            behaviour: Behaviour::Base,
            no_render: false,
            pre_tick: false,
            random: JavaRandom::new(0),
            seed: 0,
        };
        particle.set_pos(p);
        particle
    }

    /// `Particle(level, x, y, z)` without `SingleQuadParticle` (`NoRenderParticle`).
    pub fn base_at(r: &mut JavaRandom, ty: Type, p: DVec3) -> Particle {
        let mut particle = Particle::blank(ty, p);
        particle.lifetime = (4.0 / (r.next_float() * 0.9 + 0.1)) as i32;
        particle
    }

    /// `Particle(level, x, y, z)` and `SingleQuadParticle`'s size.
    pub fn at(r: &mut JavaRandom, ty: Type, p: DVec3) -> Particle {
        let mut particle = Particle::base_at(r, ty, p);
        particle.size = 0.1 * (r.next_float() * 0.5 + 0.5) * 2.0;
        particle
    }

    /// `Particle(level, x, y, z, xa, ya, za)` without `SingleQuadParticle`: `v` jittered, set to a
    /// random speed and lifted.
    pub fn base_moving(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let lifetime = (4.0 / (r.next_float() * 0.9 + 0.1)) as i32;
        let mut d = [v.x, v.y, v.z].map(|c| c + f64::from((r.next_float() * 2.0 - 1.0) * 0.4));
        let speed = f64::from((r.next_float() + r.next_float() + 1.0) * 0.15);
        let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        for (i, c) in d.iter_mut().enumerate() {
            *c = *c / length * speed * f64::from(0.4f32);
            if i == 1 {
                *c += f64::from(0.1f32);
            }
        }
        let mut particle = Particle::blank(ty, p);
        particle.lifetime = lifetime;
        particle.velocity = DVec3::from(d);
        particle
    }

    pub fn moving(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let mut particle = Particle::base_moving(r, ty, p, v);
        particle.size = 0.1 * (r.next_float() * 0.5 + 0.5) * 2.0;
        particle
    }

    /// `SingleQuadParticle.scale`: the quad and the box.
    pub fn scale(&mut self, scale: f32) {
        self.size *= scale;
        self.set_size(0.2 * scale, 0.2 * scale);
    }

    pub fn set_size(&mut self, w: f32, h: f32) {
        if w != self.bb_width || h != self.bb_height {
            self.bb_width = w;
            self.bb_height = h;
            let x = (self.bb_min.x + self.bb_max.x - f64::from(w)) / 2.0;
            let z = (self.bb_min.z + self.bb_max.z - f64::from(w)) / 2.0;
            self.bb_min = DVec3::new(x, self.bb_min.y, z);
            self.bb_max = DVec3::new(x + f64::from(w), self.bb_min.y + f64::from(h), z + f64::from(w));
        }
    }

    pub fn set_pos(&mut self, p: DVec3) {
        self.position = p;
        let half = f64::from(self.bb_width) / 2.0;
        self.bb_min = DVec3::new(p.x - half, p.y, p.z - half);
        self.bb_max = DVec3::new(p.x + half, p.y + f64::from(self.bb_height), p.z + half);
    }

    fn set_location_from_bb(&mut self) {
        self.position = DVec3::new((self.bb_min.x + self.bb_max.x) / 2.0, self.bb_min.y, (self.bb_min.z + self.bb_max.z) / 2.0);
    }

    pub fn set_color(&mut self, r: f32, g: f32, b: f32) {
        self.rgba[..3].copy_from_slice(&[r, g, b]);
    }

    /// `Particle.move` with `Entity.collideBoundingBox` against the blocks' collision boxes.
    pub fn move_by(&mut self, blocks: &impl Blocks, d: DVec3) {
        if self.free_move {
            self.bb_min += d;
            self.bb_max += d;
            self.set_location_from_bb();
            return;
        }
        if self.stopped_by_collision {
            return;
        }
        let mut moved = d;
        if self.has_physics && d != DVec3::ZERO && d.length_squared() < 100.0 * 100.0 {
            moved = level::collide(blocks, self.bb_min, self.bb_max, d);
        }
        if moved != DVec3::ZERO {
            self.bb_min += moved;
            self.bb_max += moved;
            self.set_location_from_bb();
        }
        if d.y.abs() >= f64::from(1.0e-5f32) && moved.y.abs() < f64::from(1.0e-5f32) {
            self.stopped_by_collision = true;
        }
        self.on_ground = d.y != moved.y && d.y < 0.0;
        if d.x != moved.x {
            self.velocity.x = 0.0;
        }
        if d.z != moved.z {
            self.velocity.z = 0.0;
        }
    }

    /// `Particle.tick`.
    pub fn base_tick(&mut self, blocks: &impl Blocks) {
        self.previous = self.position;
        let age = self.age;
        self.age += 1;
        if age >= self.lifetime {
            self.removed = true;
            return;
        }
        self.velocity.y -= 0.04 * f64::from(self.gravity);
        self.move_by(blocks, self.velocity);
        if self.speed_up_when_y_blocked && self.position.y == self.previous.y {
            self.velocity.x *= 1.1;
            self.velocity.z *= 1.1;
        }
        self.velocity *= f64::from(self.friction);
        if self.on_ground {
            self.velocity.x *= f64::from(0.7f32);
            self.velocity.z *= f64::from(0.7f32);
        }
    }

    /// `getQuadSize(a)`.
    pub fn quad_size(&self, a: f32) -> f32 {
        let t = (self.age as f32 + a) / self.lifetime as f32;
        self.size
            * match self.size_curve {
                Size::Fixed => 1.0,
                Size::GrowIn => (t * 32.0).clamp(0.0, 1.0),
                Size::Flame => 1.0 - t * t * 0.5,
                Size::Lava => 1.0 - t * t,
                Size::Portal => 1.0 - (1.0 - t) * (1.0 - t),
                Size::ReversePortal => 1.0 - (self.age as f32 + a) / (self.lifetime as f32 * 1.5),
            }
    }

    /// `getLightCoords(a)` as (block, sky) in light levels, from the cell's own.
    pub fn light(&self, a: f32, [block, sky]: [f32; 2]) -> [f32; 2] {
        let t = ((self.age as f32 + a) / self.lifetime as f32).clamp(0.0, 1.0);
        // Emission adds in 1/16 levels, truncated like the packed coordinates.
        let add = |share: f32| ((block * 16.0) as u32 + (share * 240.0) as u32).min(240) as f32 / 16.0;
        match self.glow {
            Glow::World => [block, sky],
            Glow::FullBright => [15.0, 15.0],
            Glow::Block15 => [15.0, sky],
            Glow::AgeEmission => [add(t), sky],
            Glow::AgeEmission4 => {
                let b = self.age as f32 / self.lifetime as f32;
                let b = (b * b) * (b * b);
                [add(b.clamp(0.0, 1.0)), sky]
            }
        }
    }
}
