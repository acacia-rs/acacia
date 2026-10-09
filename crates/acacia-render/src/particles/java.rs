//! Java's particles (26.3 client): each kind as its class's constructor and provider set it up.
//! `a` is `addParticle`'s velocity, which some providers read as a parameter instead.

use glam::{DVec3, Vec3};

use super::kind::Kind;
use super::rng::Rng;
use super::sheet::Set;
use super::sprite::{Behaviour, Blend, Frame, Grow, Light, Sprite};

const WATER: [f32; 4] = [0.2, 0.3, 1.0, 1.0];
const LAVA: [f32; 4] = [1.0, 0.2857143, 0.083333336, 1.0];

/// `Particle`'s 7-argument constructor: `a` plus noise, set to a random speed and lifted.
fn scatter(rng: &mut Rng, a: Vec3) -> Vec3 {
    let v = a + Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 0.8 - 0.4;
    let speed = (rng.f32() + rng.f32() + 1.0) * 0.15;
    v.normalize_or_zero() * speed * 0.4 + Vec3::Y * 0.1
}

/// `SingleQuadParticle`'s quad size, and `Particle`'s default lifetime.
fn quad(rng: &mut Rng) -> f32 {
    0.1 * (rng.f32() * 0.5 + 0.5) * 2.0
}

fn life(rng: &mut Rng, ticks: f32) -> u32 {
    (ticks / (rng.f32() * 0.8 + 0.2)) as u32
}

fn grey(c: f32) -> [f32; 4] {
    [c, c, c, 1.0]
}

pub fn sprite(kind: Kind, at: DVec3, a: Vec3, rng: &mut Rng) -> Option<Sprite> {
    let mut s = Sprite::new(Set::Generic, at, Vec3::ZERO, quad(rng), (4.0 / (rng.f32() * 0.9 + 0.1)) as u32);
    match kind {
        Kind::Flame | Kind::SmallFlame | Kind::SoulFlame => {
            s.set = if kind == Kind::SoulFlame { Set::SoulFlame } else { Set::Flame };
            s.velocity = scatter(rng, a) * 0.01 + a;
            s.position += DVec3::new(f64::from(rng.f32() - rng.f32()), f64::from(rng.f32() - rng.f32()), f64::from(rng.f32() - rng.f32())) * 0.05;
            s.lifetime = life(rng, 8.0) + 4;
            (s.friction, s.physics, s.grow, s.light) = (0.96, false, Grow::Flame, Light::Ramp);
            if kind == Kind::SmallFlame {
                s.size *= 0.5;
            }
        }
        Kind::Smoke | Kind::LargeSmoke => {
            let scale = if kind == Kind::LargeSmoke { 2.5 } else { 1.0 };
            s.velocity = scatter(rng, Vec3::ZERO) * 0.1 + a;
            s.color = grey(rng.f32() * 0.3);
            s.size *= 0.75 * scale;
            s.lifetime = ((8.0 / (rng.f32() * 0.8 + 0.2) * scale) as u32).max(1);
            (s.friction, s.accel, s.speed_up, s.grow, s.frame) = (0.96, Vec3::Y * 0.004, true, Grow::Ramp, Frame::Backwards);
        }
        Kind::CampfireSmoke | Kind::SignalSmoke => {
            s.set = Set::BigSmoke;
            s.frame = Frame::Fixed(rng.below(12) as u8);
            s.size *= 3.0;
            s.width = 0.25;
            s.lifetime = rng.below(50) + if kind == Kind::SignalSmoke { 280 } else { 80 };
            s.velocity = Vec3::new(a.x, a.y + rng.f32() / 500.0, a.z);
            (s.accel, s.friction, s.blend, s.behaviour) = (Vec3::Y * -3.0e-6, 1.0, Blend::Alpha, Behaviour::Campfire);
            s.color[3] = if kind == Kind::SignalSmoke { 0.95 } else { 0.9 };
        }
        Kind::Crit | Kind::EnchantedHit => {
            s.set = if kind == Kind::Crit { Set::Crit } else { Set::EnchantedHit };
            s.velocity = scatter(rng, Vec3::ZERO) * 0.1 + a * 0.4;
            s.color = grey(rng.f32() * 0.3 + 0.6);
            s.size *= 0.75;
            s.lifetime = ((6.0 / (rng.f32() * 0.8 + 0.6)) as u32).max(1);
            (s.friction, s.accel, s.physics, s.grow, s.behaviour) = (0.7, Vec3::Y * -0.02, false, Grow::Ramp, Behaviour::Crit);
            if kind == Kind::EnchantedHit {
                s.color[0] *= 0.3;
                s.color[1] *= 0.8;
            }
        }
        Kind::Heart | Kind::AngryVillager => {
            s.set = if kind == Kind::Heart { Set::Heart } else { Set::Angry };
            s.velocity = scatter(rng, Vec3::ZERO) * 0.01 + Vec3::Y * 0.1;
            s.size *= 1.5;
            s.lifetime = 16;
            (s.speed_up, s.friction, s.physics, s.grow) = (true, 0.86, false, Grow::Ramp);
            if kind == Kind::AngryVillager {
                s.position.y += 0.5;
            }
        }
        Kind::HappyVillager => {
            s.set = Set::Happy;
            s.velocity = scatter(rng, a) * 0.02;
            s.size *= rng.f32() * 0.6 + 0.5;
            s.lifetime = life(rng, 20.0);
            (s.friction, s.physics, s.width) = (0.99, false, 0.02);
        }
        Kind::Explosion => {
            s.set = Set::Explosion;
            s.lifetime = 6 + rng.below(4);
            s.color = grey(rng.f32() * 0.6 + 0.4);
            s.size = 2.0 * (1.0 - a.x * 0.5);
            (s.frame, s.light, s.behaviour) = (Frame::Age, Light::Full, Behaviour::Still);
        }
        Kind::ExplosionEmitter => (s.lifetime, s.behaviour) = (8, Behaviour::Seed),
        Kind::Poof => {
            s.velocity = a + (Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 2.0 - 1.0) * 0.05;
            s.color = grey(rng.f32() * 0.3 + 0.7);
            s.size = 0.1 * (rng.f32() * rng.f32() * 6.0 + 1.0);
            s.lifetime = life(rng, 16.0) + 2;
            (s.accel, s.friction, s.frame) = (Vec3::Y * 0.004, 0.9, Frame::Backwards);
        }
        Kind::Rain | Kind::Splash => {
            s.set = Set::Splash;
            s.frame = Frame::Fixed(rng.below(4) as u8);
            let v = scatter(rng, Vec3::ZERO);
            s.velocity = Vec3::new(v.x * 0.3, rng.f32() * 0.2 + 0.1, v.z * 0.3);
            s.lifetime = life(rng, 8.0);
            let gravity = if kind == Kind::Splash { 0.04 } else { 0.06 };
            (s.width, s.accel, s.behaviour) = (0.01, Vec3::Y * -gravity, Behaviour::Drop);
            if kind == Kind::Splash && a.y == 0.0 && (a.x != 0.0 || a.z != 0.0) {
                s.velocity = Vec3::new(a.x, 0.1, a.z);
            }
        }
        Kind::Bubble => {
            s.set = Set::Bubble;
            s.size *= rng.f32() * 0.6 + 0.2;
            s.velocity = a * 0.2 + (Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 2.0 - 1.0) * 0.02;
            s.lifetime = life(rng, 8.0);
            (s.width, s.accel, s.friction, s.behaviour) = (0.02, Vec3::Y * 0.002, 0.85, Behaviour::Bubble);
        }
        Kind::Lava => {
            s.set = Set::Lava;
            s.velocity = scatter(rng, Vec3::ZERO) * 0.8;
            s.velocity.y = rng.f32() * 0.4 + 0.05;
            s.size *= rng.f32() * 2.0 + 0.2;
            s.lifetime = life(rng, 16.0);
            (s.accel, s.friction, s.grow, s.light, s.behaviour) = (Vec3::Y * -0.03, 0.999, Grow::Lava, Light::Block, Behaviour::Lava);
        }
        Kind::Dust(c) => {
            s.velocity = scatter(rng, a) * 0.1;
            s.size *= 0.75;
            s.lifetime = life(rng, 8.0).max(1);
            let k = rng.f32() * 0.4 + 0.6;
            let [r, g, b] = c.map(|channel| (rng.f32() * 0.2 + 0.8) * channel * k);
            s.color = [r, g, b, 1.0];
            (s.friction, s.speed_up, s.frame, s.grow) = (0.96, true, Frame::Backwards, Grow::Ramp);
        }
        Kind::Note(pitch) => {
            s.set = Set::Note;
            s.velocity = scatter(rng, Vec3::ZERO) * 0.01 + Vec3::Y * 0.2;
            s.color = note_color(pitch);
            s.size *= 1.5;
            s.lifetime = 6;
            (s.friction, s.speed_up, s.grow) = (0.66, true, Grow::Ramp);
        }
        Kind::Portal => {
            s.size = 0.1 * (rng.f32() * 0.2 + 0.5);
            let k = rng.f32() * 0.6 + 0.4;
            s.color = [k * 0.9, k * 0.3, k, 1.0];
            s.lifetime = (rng.f32() * 10.0) as u32 + 40;
            s.velocity = a;
            s.frame = Frame::Fixed(rng.below(8) as u8);
            (s.physics, s.grow, s.light, s.behaviour) = (false, Grow::Portal, Light::Ramp, Behaviour::Portal { start: at });
        }
        Kind::DrippingWater | Kind::DrippingLava => {
            let lava = kind == Kind::DrippingLava;
            s.set = Set::DripHang;
            s.color = if lava { [1.0; 4] } else { WATER };
            s.lifetime = 40;
            let next = if lava { Kind::FallingLava } else { Kind::FallingWater };
            (s.width, s.accel, s.behaviour) = (0.01, Vec3::Y * -0.0012, Behaviour::Hang { next, cooling: lava });
        }
        Kind::FallingWater | Kind::FallingLava => {
            s.set = Set::DripFall;
            s.color = if kind == Kind::FallingLava { LAVA } else { WATER };
            s.lifetime = life(rng, 64.0);
            let land = if kind == Kind::FallingLava { Kind::LandingLava } else { Kind::Splash };
            (s.width, s.accel, s.behaviour) = (0.01, Vec3::Y * -0.06, Behaviour::Fall { land });
        }
        Kind::LandingLava => {
            s.set = Set::DripLand;
            s.lifetime = life(rng, 16.0);
            (s.color, s.width, s.accel) = (LAVA, 0.01, Vec3::Y * -0.06);
        }
        Kind::CritBurst | Kind::EnchantedHitBurst => return None,
    }
    Some(s)
}

/// Java's `NoteParticle` colour for a pitch 0 to 1, shared by both looks.
pub fn note_color(pitch: f32) -> [f32; 4] {
    let channel = |offset: f32| ((pitch + offset) * std::f32::consts::TAU).sin().mul_add(0.65, 0.35).max(0.0);
    [channel(0.0), channel(1.0 / 3.0), channel(2.0 / 3.0), 1.0]
}
