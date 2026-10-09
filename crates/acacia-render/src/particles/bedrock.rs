//! Bedrock's particles, from the vanilla pack's `particles/*.json` (1.26.50): sizes, lifetimes,
//! speeds, accelerations, drag and tints, put into game ticks. Velocities there are blocks a
//! second and accelerations blocks a second², integrated as `v += (a - drag·v)·dt`, so a tick
//! adds `a/400` and keeps `1 - drag/20` of the velocity.

use glam::{DVec3, Vec3};

use super::java::note_color;
use super::kind::Kind;
use super::rng::Rng;
use super::sheet::Set;
use super::sprite::{Behaviour, Frame, Grow, Light, Sprite};

const WATER: [f32; 4] = [0.2, 0.3, 1.0, 1.0];

fn seconds(rng: &mut Rng, low: f32, high: f32) -> u32 {
    (rng.range(low, high) * 20.0).round().max(1.0) as u32
}

/// `particle_initial_speed` along a normalised direction, in blocks a tick.
fn launch(direction: Vec3, speed: f32) -> Vec3 {
    direction.normalize_or_zero() * speed / 20.0
}

fn per_tick(accel: Vec3) -> Vec3 {
    accel / 400.0
}

fn drag(coefficient: f32) -> f32 {
    (1.0 - coefficient / 20.0).max(0.0)
}

fn grey(c: f32) -> [f32; 4] {
    [c, c, c, 1.0]
}

pub fn sprite(kind: Kind, at: DVec3, a: Vec3, rng: &mut Rng) -> Option<Sprite> {
    let mut s = Sprite::new(Set::Generic, at, Vec3::ZERO, 0.1, 20);
    s.friction = 1.0;
    match kind {
        Kind::Flame | Kind::SoulFlame | Kind::SmallFlame => {
            s.set = if kind == Kind::SoulFlame { Set::SoulFlame } else { Set::Flame };
            let r = rng.f32();
            (s.size, s.grow, s.lifetime) = match kind {
                Kind::SmallFlame => (0.5 * (0.1 + r * 0.1), Grow::Shrink(0.5 * 0.01 / 20.0), seconds(rng, 9.0, 11.5)),
                _ => (0.1 + r * 0.1, Grow::Shrink(0.1 / 20.0), seconds(rng, 0.6, 2.0)),
            };
            s.position += (Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 2.0 - 1.0).as_dvec3() * 0.025;
            (s.physics, s.light) = (false, Light::Full);
        }
        Kind::Smoke | Kind::LargeSmoke => {
            s.velocity = launch(Vec3::new(rng.range(-0.1, 0.1), 1.0, rng.range(-0.1, 0.1)), 1.0) + a;
            s.accel = per_tick(Vec3::Y * 0.4);
            s.size = if kind == Kind::LargeSmoke { 0.25 } else { 0.1 };
            s.lifetime = seconds(rng, 0.4, 1.4);
            s.color = grey(rng.f32() * 0.5);
            s.frame = Frame::Backwards;
        }
        Kind::CampfireSmoke | Kind::SignalSmoke => {
            s.set = Set::BigSmoke;
            s.frame = Frame::Fixed(rng.below(12) as u8);
            s.velocity = launch(Vec3::Y, rng.range(0.5, 1.0));
            s.accel = per_tick(Vec3::Y * 0.4);
            s.lifetime = if kind == Kind::SignalSmoke { 180 } else { 100 };
            (s.size, s.grow, s.width) = (0.75, Grow::Campfire, 0.5);
        }
        Kind::Crit | Kind::EnchantedHit => {
            let (r1, r2) = (rng.f32(), rng.f32());
            s.size = 0.1 + rng.f32() * 0.05;
            s.velocity = if a == Vec3::ZERO { launch(Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)), 2.0) } else { a };
            s.lifetime = ((6.0 / (rng.range(0.0, 16.0) + 12.0)) * 20.0).round().max(1.0) as u32;
            s.friction = drag(10.0);
            s.physics = false;
            if kind == Kind::Crit {
                (s.set, s.frame, s.accel) = (Set::Crit, Frame::Age, per_tick(Vec3::Y * -10.0));
                (s.color, s.fade) = ([r1 * 0.3 + 0.6, r2 * 0.3 + 0.6, r2 * 0.3 + 0.6, 1.0], Some([r1 * 0.3 + 0.6, 0.5, 0.3]));
            } else {
                (s.set, s.accel) = (Set::EnchantedHit, per_tick(Vec3::Y * -12.0));
                (s.color, s.fade) = ([r1 * 0.3 + 0.6, r2 * 0.3 + 0.6, 0.8, 1.0], Some([0.5, 0.5, 0.8]));
            }
        }
        Kind::Explosion => {
            s.set = Set::Explosion;
            s.lifetime = seconds(rng, 0.3, 0.5);
            s.color = grey(rng.f32() * 0.6 + 0.4);
            (s.size, s.frame, s.light, s.behaviour) = (1.0, Frame::Age, Light::Full, Behaviour::Still);
        }
        Kind::ExplosionEmitter => (s.lifetime, s.behaviour) = (8, Behaviour::Seed),
        Kind::Poof => {
            let r1 = rng.f32();
            let direction = if a == Vec3::ZERO { Vec3::Y } else { a.normalize() };
            s.velocity = (direction * 10.0 + Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0))) / 20.0;
            s.size = r1 * rng.f32() * 0.6 + 0.1;
            s.lifetime = ((4.0 / rng.range(1.0, 5.0) + 0.1) * 20.0) as u32;
            s.color = grey(r1 * 0.3 + 0.7);
            (s.accel, s.friction, s.frame) = (per_tick(Vec3::Y * 2.0), drag(2.5), Frame::Backwards);
        }
        Kind::Heart | Kind::AngryVillager => {
            s.set = if kind == Kind::Heart { Set::Heart } else { Set::Angry };
            s.velocity = launch(Vec3::Y, 2.5);
            s.size = if kind == Kind::Heart { 0.2 + rng.f32() * 0.1 } else { 0.14 };
            s.lifetime = seconds(rng, 0.2, 1.5);
            (s.friction, s.physics) = (drag(5.0), false);
        }
        Kind::HappyVillager => {
            s.set = Set::Happy;
            s.lifetime = seconds(rng, 1.0, 5.0);
            s.accel = per_tick(Vec3::new(rng.range(-0.02, 0.02), rng.range(-0.02, 0.02), rng.range(-0.02, 0.02)));
            (s.size, s.physics) = (0.14, false);
        }
        Kind::Splash | Kind::Rain => {
            // Drag 20 leaves only this tick's acceleration, which falls 410 (rain 480) a second².
            s.set = Set::Splash;
            s.frame = Frame::Fixed(rng.below(4) as u8);
            let (sideways, falls, size, lifetime) = match kind {
                Kind::Splash => (6.0, 410.0, 0.2, seconds(rng, 0.4, 2.0)),
                _ => (0.0, 480.0, 0.175, seconds(rng, 0.19, 0.21)),
            };
            s.velocity = Vec3::new(sideways, 30.0 + rng.f32() * 60.0, sideways) / 400.0 + a;
            s.accel = Vec3::Y * -(falls / 20.0 / 400.0);
            (s.size, s.lifetime, s.width, s.behaviour) = (size, lifetime, 0.02, Behaviour::Drop);
        }
        Kind::Bubble => {
            s.set = Set::Bubble;
            s.velocity = a * 0.2 + (Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 2.0 - 1.0) * 0.001;
            s.size = (0.05 * rng.f32() + 0.1) * (rng.f32() * 0.9 + 0.2);
            s.lifetime = ((2.0 / ((rng.f32() * 0.8 + 0.2) * 5.0)) * 20.0) as u32;
            (s.accel, s.friction, s.width, s.behaviour) = (per_tick(Vec3::Y * 0.8), drag(5.25), 0.02, Behaviour::Bubble);
        }
        Kind::Lava => {
            s.set = Set::Lava;
            s.velocity = launch(Vec3::new(rng.range(-0.1, 0.1), rng.range(0.1, 1.0), rng.range(-0.1, 0.1)), rng.range(5.0, 10.0));
            s.accel = per_tick(Vec3::new(rng.f32() * 5.0 - 2.5, -15.0, rng.f32() * 5.0 - 2.5));
            s.size = rng.f32() * 0.15 + 0.025;
            s.lifetime = seconds(rng, 0.5, 2.0);
            (s.grow, s.light, s.behaviour) = (Grow::Lava, Light::Full, Behaviour::Lava);
        }
        Kind::Dust(c) => {
            s.velocity = Vec3::new(rng.range(-0.4, 0.4), rng.range(-0.1, 0.1), rng.range(-0.4, 0.4)) / 20.0;
            let shade = (rng.f32() * 0.2 + 0.8) * (rng.f32() * 0.4 + 0.6);
            s.size = rng.f32() * 0.075 + 0.075;
            s.lifetime = ((2.0 / rng.range(1.0, 5.0)) * 20.0) as u32;
            s.color = [c[0] * shade, c[1] * shade, c[2] * shade, 1.0];
            s.frame = Frame::Backwards;
        }
        Kind::Note(pitch) => {
            s.set = Set::Note;
            s.velocity = launch(Vec3::Y, 2.5);
            s.size = 0.1 + rng.f32() * 0.05;
            (s.color, s.lifetime, s.friction) = (note_color(pitch), 6, drag(2.5));
        }
        Kind::Portal => {
            let r1 = rng.f32();
            s.frame = Frame::Fixed((r1 * 8.0).round().min(7.0) as u8);
            s.size = rng.f32() * 0.02 + 0.05;
            s.lifetime = seconds(rng, 2.0, 2.45);
            let k = r1 * 0.6 + 0.4;
            s.color = [k, k * 0.3, k * 0.9, 1.0];
            // Sinks a block over its life: the Java formula from a block lower, standing still.
            let start = at - DVec3::Y;
            (s.physics, s.grow, s.light, s.behaviour) = (false, Grow::Portal, Light::Full, Behaviour::Portal { start });
        }
        Kind::DrippingWater | Kind::DrippingLava => {
            let lava = kind == Kind::DrippingLava;
            s.set = Set::DripHang;
            s.position.y += 0.1;
            s.color = if lava { [1.0; 4] } else { WATER };
            let next = if lava { Kind::FallingLava } else { Kind::FallingWater };
            (s.size, s.lifetime, s.width) = (0.15, 40, 0.02);
            s.behaviour = Behaviour::Hang { next, cooling: lava };
            s.light = if lava { Light::Full } else { Light::World };
        }
        Kind::FallingWater | Kind::FallingLava | Kind::LandingLava => {
            let lava = kind != Kind::FallingWater;
            s.set = Set::DripFall;
            s.color = if lava { [1.0, 0.8 / 2.8, 0.2 / 2.4, 1.0] } else { WATER };
            s.accel = per_tick(Vec3::Y * -19.6);
            (s.size, s.lifetime, s.width) = (0.15, 60, 0.02);
            if kind != Kind::LandingLava {
                s.behaviour = Behaviour::Fall { land: if lava { Kind::LandingLava } else { Kind::Rain } };
            }
            s.light = if lava { Light::Full } else { Light::World };
        }
        Kind::CritBurst | Kind::EnchantedHitBurst => return None,
    }
    Some(s)
}
