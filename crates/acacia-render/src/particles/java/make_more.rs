//! `ParticleProvider.createParticle` for portal and enchant glyphs, crits, villager and town
//! motes, explosions, campfire smoke, notes, dust, end rods, leaves and falling dust.

use glam::DVec3;

use super::Engine;
use super::behaviour::{Behaviour, Options};
use super::particle::{Glow, Particle, Size};
use super::random::{JavaRandom, mth_sin};
use super::types::Type;

fn f(v: f32) -> f64 {
    f64::from(v)
}

impl Engine {
    pub(super) fn portal(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::at(r, ty, p);
        q.velocity = v;
        q.size = 0.1 * (r.float() * 0.2 + 0.5);
        let br = r.float() * 0.6 + 0.4;
        q.set_color(br * 0.9, br * 0.3, br);
        q.lifetime = (r.float() * 10.0) as i32 + 40;
        q.free_move = true;
        q.glow = Glow::AgeEmission4;
        if ty == Type::Portal {
            q.size_curve = Size::Portal;
            q.behaviour = Behaviour::Portal { start: p };
        } else {
            q.size *= 1.5;
            q.lifetime = (r.float() * 2.0) as i32 + 60;
            q.size_curve = Size::ReversePortal;
            q.behaviour = Behaviour::ReversePortal;
        }
        q
    }

    /// `FlyTowardsPositionParticle` (enchanting table glyphs).
    pub(super) fn enchant(r: &mut JavaRandom, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::at(r, Type::Enchant, p);
        let alpha = [1.0, 1.0, 0.0, 1.0];
        q.rgba[3] = alpha[0];
        q.velocity = v;
        q.set_pos(p + v);
        q.previous = p + v;
        q.size = 0.1 * (r.float() * 0.5 + 0.2);
        let br = r.float() * 0.6 + 0.4;
        q.set_color(0.9 * br, 0.9 * br, br);
        q.has_physics = false;
        q.lifetime = (r.float() * 10.0) as i32 + 30;
        q.free_move = true;
        q.glow = Glow::AgeEmission4;
        q.behaviour = Behaviour::FlyTowards { start: p, alpha };
        q
    }

    /// `CritParticle`: its constructor ticks it once (see `Engine::admit`).
    pub(super) fn crit(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::moving(r, ty, p, DVec3::ZERO);
        q.friction = 0.7;
        q.gravity = 0.5;
        q.velocity = q.velocity * f(0.1) + v * 0.4;
        let col = r.float() * 0.3 + 0.6;
        q.set_color(col, col, col);
        q.size *= 0.75;
        q.lifetime = ((6.0 / (f(r.float()) * 0.8 + 0.6)) as i32).max(1);
        q.has_physics = false;
        q.size_curve = Size::GrowIn;
        q.behaviour = Behaviour::Crit;
        q.pre_tick = true;
        q
    }

    pub(super) fn heart(r: &mut JavaRandom, ty: Type, p: DVec3) -> Particle {
        let at = if ty == Type::AngryVillager { p + DVec3::new(0.0, 0.5, 0.0) } else { p };
        let mut q = Particle::moving(r, ty, at, DVec3::ZERO);
        q.speed_up_when_y_blocked = true;
        q.friction = 0.86;
        q.velocity *= f(0.01);
        q.velocity.y += 0.1;
        q.size *= 1.5;
        q.lifetime = 16;
        q.has_physics = false;
        q.size_curve = Size::GrowIn;
        q
    }

    /// `SuspendedTownParticle`; the provider picks the sprite first.
    pub(super) fn town(&mut self, r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let sprite = self.sprite_from_set(ty);
        let mut q = Particle::moving(r, ty, p, v);
        q.sprite = sprite;
        let br = r.float() * 0.1 + 0.2;
        q.set_color(br, br, br);
        q.set_size(0.02, 0.02);
        q.size *= r.float() * 0.6 + 0.5;
        q.velocity *= f(0.02);
        q.lifetime = (20.0 / (f(r.float()) * 0.8 + 0.2)) as i32;
        q.free_move = true;
        q.behaviour = Behaviour::SuspendedTown;
        if ty == Type::HappyVillager {
            q.set_color(1.0, 1.0, 1.0);
        }
        q
    }

    /// `ExplodeParticle` (poof).
    pub(super) fn poof(r: &mut JavaRandom, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::at(r, Type::Poof, p);
        q.gravity = -0.1;
        q.friction = 0.9;
        q.velocity = DVec3::new(v.x + f((r.float() * 2.0 - 1.0) * 0.05), v.y + f((r.float() * 2.0 - 1.0) * 0.05), v.z + f((r.float() * 2.0 - 1.0) * 0.05));
        let col = r.float() * 0.3 + 0.7;
        q.set_color(col, col, col);
        q.size = 0.1 * (r.float() * r.float() * 6.0 + 1.0);
        q.lifetime = (16.0 / (f(r.float()) * 0.8 + 0.2)) as i32 + 2;
        q.behaviour = Behaviour::Animated;
        q.animated = true;
        q
    }

    /// `HugeExplosionParticle` and its seed, which draws nothing.
    pub(super) fn explosion(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        if ty == Type::ExplosionEmitter {
            let mut q = Particle::base_moving(r, ty, p, DVec3::ZERO);
            q.lifetime = 8;
            q.no_render = true;
            q.behaviour = Behaviour::ExplosionSeed;
            return q;
        }
        let mut q = Particle::moving(r, ty, p, DVec3::ZERO);
        q.lifetime = 6 + r.below(4);
        let col = r.float() * 0.6 + 0.4;
        q.set_color(col, col, col);
        q.size = 2.0 * (1.0 - v.x as f32 * 0.5);
        q.glow = Glow::FullBright;
        q.behaviour = Behaviour::HugeExplosion;
        q.animated = true;
        q
    }

    pub(super) fn campfire(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let signal = ty == Type::CampfireSignalSmoke;
        let mut q = Particle::at(r, ty, p);
        q.scale(3.0);
        q.set_size(0.25, 0.25);
        q.lifetime = r.below(50) + if signal { 280 } else { 80 };
        q.gravity = 3.0e-6;
        q.velocity = DVec3::new(v.x, v.y + f(r.float() / 500.0), v.z);
        q.rgba[3] = if signal { 0.95 } else { 0.9 };
        q.behaviour = Behaviour::Campfire;
        q
    }

    /// `NoteParticle`: `xa` is the note's colour, 0 to 1.
    pub(super) fn note(r: &mut JavaRandom, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::moving(r, Type::Note, p, DVec3::ZERO);
        q.friction = 0.66;
        q.speed_up_when_y_blocked = true;
        q.velocity *= f(0.01);
        q.velocity.y += 0.2;
        let color = v.x as f32;
        let channel = |offset: f32| (mth_sin(f((color + offset) * std::f64::consts::TAU as f32)) * 0.65 + 0.35).max(0.0);
        q.set_color(channel(0.0), channel(0.333_333_34), channel(0.666_666_7));
        q.size *= 1.5;
        q.lifetime = 6;
        q.size_curve = Size::GrowIn;
        q
    }

    /// `DustParticle` and `DustColorTransitionParticle` (`DustParticleBase`).
    pub(super) fn dust(r: &mut JavaRandom, o: &Options, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::moving(r, o.ty, p, v);
        q.friction = 0.96;
        q.speed_up_when_y_blocked = true;
        q.velocity *= f(0.1);
        q.size *= 0.75 * o.scale;
        let base = (8.0 / (r.double() * 0.8 + 0.2)) as i32;
        q.lifetime = (base as f32 * o.scale).max(1.0) as i32;
        let factor = r.float() * 0.4 + 0.6;
        let mut randomize = |c: f32| (r.float() * 0.2 + 0.8) * c * factor;
        let from = [randomize(o.color[0]), randomize(o.color[1]), randomize(o.color[2])];
        q.size_curve = Size::GrowIn;
        q.animated = true;
        if o.ty == Type::DustColorTransition {
            let to = [randomize(o.to_color[0]), randomize(o.to_color[1]), randomize(o.to_color[2])];
            q.behaviour = Behaviour::Transition { from, to };
        } else {
            q.set_color(from[0], from[1], from[2]);
            q.behaviour = Behaviour::Animated;
        }
        q
    }

    /// `EndRodParticle` and `TotemParticle` (`SimpleAnimatedParticle`).
    pub(super) fn glitter(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let totem = ty == Type::TotemOfUndying;
        let mut q = Particle::at(r, ty, p);
        q.friction = 0.91;
        q.gravity = if totem { 1.25 } else { 0.0125 };
        q.velocity = v;
        q.size *= 0.75;
        q.lifetime = 60 + r.below(12);
        let mut fade = None;
        if totem {
            q.friction = 0.6;
            if r.below(4) == 0 {
                q.set_color(0.6 + r.float() * 0.2, 0.6 + r.float() * 0.3, r.float() * 0.2);
            } else {
                q.set_color(0.1 + r.float() * 0.2, 0.4 + r.float() * 0.3, r.float() * 0.2);
            }
        } else {
            fade = Some([0xF2 as f32 / 255.0, 0xDE as f32 / 255.0, 0xC9 as f32 / 255.0]);
            q.free_move = true;
        }
        q.glow = Glow::FullBright;
        q.behaviour = Behaviour::SimpleAnimated { fade };
        q.animated = true;
        q
    }

    /// `FallingLeavesParticle`.
    pub(super) fn leaves(r: &mut JavaRandom, o: &Options, p: DVec3) -> Particle {
        let ty = o.ty;
        let (fall, side, swirl, flow, scale, start) =
            if ty == Type::CherryLeaves { (0.25f32, 2.0f32, false, true, 1.0f32, 0.0f32) } else { (0.07, 10.0, true, false, 2.0, 0.021) };
        let mut q = Particle::at(r, ty, p);
        let rot_speed = (if r.boolean() { -30.0f32 } else { 30.0 }).to_radians();
        let spin = (if r.boolean() { -5.0f32 } else { 5.0 }).to_radians();
        q.lifetime = 300;
        q.gravity = fall * 1.2 * 0.0025;
        let s = scale * if r.boolean() { 0.05 } else { 0.075 };
        q.size = s;
        q.set_size(s, s);
        q.friction = 1.0;
        q.velocity.y = -f(start);
        let pr = r.float();
        let flow_scale = [f(pr * 60.0).to_radians().cos() * f(side), f(pr * 60.0).to_radians().sin() * f(side)];
        let swirl_period = f(1000.0 + pr * 3000.0).to_radians();
        if ty == Type::TintedLeaves {
            q.set_color(o.color[0], o.color[1], o.color[2]);
        }
        q.behaviour = Behaviour::Leaves { rot_speed, spin, wind: side, swirl, flow, flow_scale, swirl_period };
        q
    }

    /// `FallingDustParticle`, tinted by the block's dust colour.
    pub(super) fn falling_dust(r: &mut JavaRandom, o: &Options, p: DVec3) -> Particle {
        let mut q = Particle::at(r, Type::FallingDust, p);
        q.set_color(o.color[0], o.color[1], o.color[2]);
        q.size *= 0.674_999_95;
        let base = (32.0 / (r.float() * 0.8 + 0.2)) as i32;
        q.lifetime = ((base as f32 * 0.9) as i32).max(1);
        let rot_speed = (r.float() - 0.5) * 0.1;
        q.roll = r.float() * std::f32::consts::TAU;
        q.size_curve = Size::GrowIn;
        q.behaviour = Behaviour::FallingDust { rot_speed };
        q.animated = true;
        q
    }
}
