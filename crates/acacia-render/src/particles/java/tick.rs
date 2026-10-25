//! Each Java particle class's `tick`.

use glam::{DVec3, IVec3};

use super::Engine;
use super::behaviour::{Behaviour, Options};
use super::level::{self, Fluid};
use super::particle::Particle;
use super::types::Type;
use crate::particles::collide::Blocks;

fn cell(p: DVec3) -> IVec3 {
    p.floor().as_ivec3()
}

fn f(v: f32) -> f64 {
    f64::from(v)
}

/// The prologue of the ticks that count age up themselves: false once the particle is gone.
fn age_up(p: &mut Particle) -> bool {
    p.previous = p.position;
    let age = p.age;
    p.age += 1;
    if age >= p.lifetime {
        p.removed = true;
    }
    !p.removed
}

/// The prologue of the ticks that count the lifetime down instead.
fn count_down(p: &mut Particle) -> bool {
    p.previous = p.position;
    let lifetime = p.lifetime;
    p.lifetime -= 1;
    if lifetime <= 0 {
        p.removed = true;
    }
    !p.removed
}

impl Engine {
    pub(super) fn tick_particle(&mut self, p: &mut Particle, blocks: &impl Blocks) {
        match p.behaviour {
            Behaviour::Base => p.base_tick(blocks),
            Behaviour::Animated | Behaviour::Transition { .. } => {
                p.base_tick(blocks);
                p.sprite_from_age();
            }
            Behaviour::Lava => {
                p.base_tick(blocks);
                if !p.removed && p.random.float() > p.age as f32 / p.lifetime as f32 {
                    self.simple(Type::Smoke, p.position, p.velocity);
                }
            }
            Behaviour::Crit => {
                p.base_tick(blocks);
                p.rgba[1] *= 0.96;
                p.rgba[2] *= 0.9;
            }
            Behaviour::Drip { fluid, stage, next, sound } => self.drip_tick(p, blocks, fluid, stage, next, sound),
            Behaviour::WaterDrop => {
                if !count_down(p) {
                    return;
                }
                p.velocity.y -= f(p.gravity);
                p.move_by(blocks, p.velocity);
                p.velocity *= f(0.98);
                if p.on_ground {
                    if p.random.float() < 0.5 {
                        p.removed = true;
                    }
                    p.velocity.x *= f(0.7);
                    p.velocity.z *= f(0.7);
                }
                let b = cell(p.position);
                let top = level::collision_top(blocks, b, p.position.x - f64::from(b.x), p.position.z - f64::from(b.z));
                let offset = top.max(level::fluid_height(blocks, b).1);
                if offset > 0.0 && p.position.y < f64::from(b.y) + offset {
                    p.removed = true;
                }
            }
            Behaviour::Bubble => {
                if !count_down(p) {
                    return;
                }
                p.velocity.y += 0.002;
                p.move_by(blocks, p.velocity);
                p.velocity *= f(0.85);
                if level::fluid(blocks, cell(p.position)).fluid != Fluid::Water {
                    p.removed = true;
                }
            }
            Behaviour::SulfurBubble { y_start, y_end, size_start, y_prev } => {
                p.base_tick(blocks);
                let here = level::fluid(blocks, cell(p.position));
                let gone = p.removed || !(here.fluid == Fluid::Water && here.is_source()) || p.position.y >= y_end || p.position.y <= y_prev;
                p.removed = gone;
                // randomHorizontalWiggling, then a second, horizontal move.
                let mut wiggle = || {
                    let f = p.random.float() * 0.003;
                    f64::from(if p.random.boolean() { f } else { -f }) * 0.5
                };
                let (wx, wz) = (wiggle(), wiggle());
                p.velocity.x += wx;
                p.velocity.z += wz;
                p.move_by(blocks, DVec3::new(p.velocity.x, 0.0, p.velocity.z));
                let progress = ((p.position.y - y_start) / (y_end - y_start)) as f32;
                p.size = size_start + progress * (0.15 - size_start);
                p.behaviour = Behaviour::SulfurBubble { y_start, y_end, size_start, y_prev: p.position.y };
            }
            Behaviour::BubbleColumnUp => {
                p.base_tick(blocks);
                if !p.removed && level::fluid(blocks, cell(p.position)).fluid != Fluid::Water {
                    p.removed = true;
                }
            }
            Behaviour::CurrentDown { angle } => {
                if !age_up(p) {
                    return;
                }
                p.velocity.x += f(0.6 * angle.cos());
                p.velocity.z += f(0.6 * angle.sin());
                p.velocity.x *= 0.07;
                p.velocity.z *= 0.07;
                p.move_by(blocks, p.velocity);
                if level::fluid(blocks, cell(p.position)).fluid != Fluid::Water || p.on_ground {
                    p.removed = true;
                }
                p.behaviour = Behaviour::CurrentDown { angle: angle + 0.08 };
            }
            Behaviour::SuspendedTown => {
                if count_down(p) {
                    p.move_by(blocks, p.velocity);
                    p.velocity *= 0.99;
                }
            }
            Behaviour::Portal { start } => {
                if age_up(p) {
                    let a = p.age as f32 / p.lifetime as f32;
                    let along = f(1.0 - (-a + a * a * 2.0));
                    p.position = start + p.velocity * along + DVec3::new(0.0, f(1.0 - a), 0.0);
                }
            }
            Behaviour::ReversePortal => {
                if age_up(p) {
                    p.position += p.velocity * f(p.age as f32 / p.lifetime as f32);
                }
            }
            Behaviour::FlyTowards { start, .. } => {
                if age_up(p) {
                    let along = 1.0 - p.age as f32 / p.lifetime as f32;
                    let mut drop = 1.0 - along;
                    drop *= drop;
                    drop *= drop;
                    p.position = start + p.velocity * f(along) - DVec3::new(0.0, f(drop * 1.2), 0.0);
                }
            }
            Behaviour::HugeExplosion => {
                if age_up(p) {
                    p.sprite_from_age();
                }
            }
            Behaviour::ExplosionSeed => {
                for _ in 0..6 {
                    let r = &mut p.random;
                    let mut d = || (r.double() - r.double()) * 4.0;
                    let at = p.position + DVec3::new(d(), d(), d());
                    let share = f(p.age as f32 / p.lifetime as f32);
                    self.add(&Options::simple(Type::Explosion), at, DVec3::new(share, 0.0, 0.0));
                }
                p.age += 1;
                if p.age == p.lifetime {
                    p.removed = true;
                }
            }
            Behaviour::Campfire => {
                p.previous = p.position;
                let age = p.age;
                p.age += 1;
                if age < p.lifetime && p.rgba[3] > 0.0 {
                    let r = &mut p.random;
                    let dx = r.float() / 5000.0 * if r.boolean() { 1.0 } else { -1.0 };
                    p.velocity.x += f(dx);
                    let dz = r.float() / 5000.0 * if r.boolean() { 1.0 } else { -1.0 };
                    p.velocity.z += f(dz);
                    p.velocity.y -= f(p.gravity);
                    p.move_by(blocks, p.velocity);
                    if p.age >= p.lifetime - 60 && p.rgba[3] > 0.01 {
                        p.rgba[3] -= 0.015;
                    }
                } else {
                    p.removed = true;
                }
            }
            Behaviour::SimpleAnimated { fade } => {
                p.base_tick(blocks);
                p.sprite_from_age();
                if p.age > p.lifetime / 2 {
                    p.rgba[3] = 1.0 - (p.age as f32 - (p.lifetime / 2) as f32) / p.lifetime as f32;
                    if let Some(fade) = fade {
                        (0..3).for_each(|i| p.rgba[i] += (fade[i] - p.rgba[i]) * 0.2);
                    }
                }
            }
            Behaviour::Leaves { .. } => leaves_tick(p, blocks),
            Behaviour::FallingDust { rot_speed } => {
                if !age_up(p) {
                    return;
                }
                p.sprite_from_age();
                p.o_roll = p.roll;
                p.roll += std::f32::consts::PI * rot_speed * 2.0;
                if p.on_ground {
                    p.o_roll = 0.0;
                    p.roll = 0.0;
                }
                p.move_by(blocks, p.velocity);
                p.velocity.y -= f(0.003);
                p.velocity.y = p.velocity.y.max(f(-0.14));
            }
        }
    }

}

/// `FallingLeavesParticle.tick`.
fn leaves_tick(p: &mut Particle, blocks: &impl Blocks) {
    let Behaviour::Leaves { mut rot_speed, spin, wind, swirl, flow, flow_scale, swirl_period } = p.behaviour else { return };
    if count_down(p) {
        let alive = (300 - p.lifetime) as f32;
        let relative = f((alive / 300.0).min(1.0));
        let (mut xa, mut za) = (0.0f64, 0.0f64);
        if flow {
            xa += flow_scale[0] * relative.powf(1.25);
            za += flow_scale[1] * relative.powf(1.25);
        }
        if swirl {
            xa += relative * (relative * swirl_period).cos() * f(wind);
            za += relative * (relative * swirl_period).sin() * f(wind);
        }
        p.velocity.x += xa * f(0.0025);
        p.velocity.z += za * f(0.0025);
        p.velocity.y -= f(p.gravity);
        rot_speed += spin / 20.0;
        p.o_roll = p.roll;
        p.roll += rot_speed / 20.0;
        p.move_by(blocks, p.velocity);
        if p.on_ground || p.lifetime < 299 && (p.velocity.x == 0.0 || p.velocity.z == 0.0) {
            p.removed = true;
        }
        if !p.removed {
            p.velocity *= f(p.friction);
        }
    }
    p.behaviour = Behaviour::Leaves { rot_speed, spin, wind, swirl, flow, flow_scale, swirl_period };
}
