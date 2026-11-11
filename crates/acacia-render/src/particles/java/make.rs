//! `ParticleProvider.createParticle` for smoke, flames, lava, drips, drops, bubbles and the
//! suspended motes. `r` is the particle's own random; provider draws use the engine's.

use glam::DVec3;

use super::Engine;
use super::behaviour::{Behaviour, DripStage, Options};
use super::level::Fluid;
use super::particle::{Glow, Particle, Size};
use super::JavaRandom;
use super::types::Type;

fn f(v: f32) -> f64 {
    f64::from(v)
}

impl Engine {
    /// `BaseAshSmokeParticle(dir, xa, scale, colorRandom, maxLifetime, gravity, hasPhysics)`.
    pub(super) fn smoke(&mut self, r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let (dir, scale, color_random, max_lifetime, gravity, physics, vel) = match ty {
            Type::Smoke | Type::WhiteSmoke => ([0.1, 0.1, 0.1], 1.0, 0.3, 8, -0.1, true, v),
            Type::LargeSmoke => ([0.1, 0.1, 0.1], 2.5, 0.3, 8, -0.1, true, v),
            Type::Ash => ([0.1, -0.1, 0.1], 1.0, 0.5, 20, 0.1, false, DVec3::ZERO),
            _ => {
                // WhiteAshParticle.Provider draws the motion.
                let pr = &mut self.provider;
                let xa = f(pr.next_float()) * -1.9 * f(pr.next_float()) * 0.1;
                let ya = f(pr.next_float()) * -0.5 * f(pr.next_float()) * 0.1 * 5.0;
                let za = f(pr.next_float()) * -1.9 * f(pr.next_float()) * 0.1;
                ([0.1, -0.1, 0.1], 1.0, 0.0, 20, 0.0125, false, DVec3::new(xa, ya, za))
            }
        };
        let mut q = Particle::moving(r, ty, p, DVec3::ZERO);
        q.friction = 0.96;
        q.gravity = gravity;
        q.speed_up_when_y_blocked = true;
        q.velocity = q.velocity * DVec3::from(dir.map(|d: f32| f(d))) + vel;
        let col = r.next_float() * color_random;
        q.set_color(col, col, col);
        q.size *= 0.75 * scale;
        q.lifetime = ((f64::from(max_lifetime) / (f(r.next_float()) * 0.8 + 0.2) * f(scale)) as i32).max(1);
        q.has_physics = physics;
        if matches!(ty, Type::WhiteSmoke | Type::WhiteAsh) {
            q.set_color(186.0 / 255.0, 177.0 / 255.0, 194.0 / 255.0);
        }
        q.size_curve = Size::GrowIn;
        q.behaviour = Behaviour::Animated;
        q.animated = true;
        q
    }

    /// `RisingParticle`s: the flames.
    pub(super) fn flame(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::moving(r, ty, p, v);
        q.friction = 0.96;
        q.velocity = q.velocity * f(0.01) + v;
        let jitter = DVec3::new(f((r.next_float() - r.next_float()) * 0.05), f((r.next_float() - r.next_float()) * 0.05), f((r.next_float() - r.next_float()) * 0.05));
        // The constructor moves x, y, z but not the box, which the first move snaps back to.
        q.position += jitter;
        q.lifetime = (8.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32 + 4;
        q.free_move = true;
        q.size_curve = Size::Flame;
        q.glow = Glow::AgeEmission;
        if ty == Type::SmallFlame {
            q.scale(0.5);
        }
        q
    }

    pub(super) fn lava(r: &mut JavaRandom, p: DVec3) -> Particle {
        let mut q = Particle::moving(r, Type::Lava, p, DVec3::ZERO);
        q.gravity = 0.75;
        q.friction = 0.999;
        q.velocity *= f(0.8);
        q.velocity.y = f(r.next_float() * 0.4 + 0.05);
        q.size *= r.next_float() * 2.0 + 0.2;
        q.lifetime = (16.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32;
        q.size_curve = Size::Lava;
        q.glow = Glow::Block15;
        q.behaviour = Behaviour::Lava;
        q
    }

    /// `DripParticle` and its providers, which pick the sprite first.
    pub(super) fn drip(&mut self, r: &mut JavaRandom, ty: Type, p: DVec3) -> Particle {
        use Type as T;
        let sprite = self.sprite_from_set(ty);
        let mut q = Particle::at(r, ty, p);
        q.sprite = sprite;
        q.set_size(0.01, 0.01);
        q.gravity = 0.06;
        const WATER: [f32; 3] = [0.2, 0.3, 1.0];
        const LAVA: [f32; 3] = [1.0, 0.285_714_3, 0.083_333_336];
        const TEAR: [f32; 3] = [0.511_718_75, 0.031_25, 0.890_625];
        let (fluid, stage, next, color, sound) = match ty {
            T::DrippingWater => (Fluid::Water, DripStage::Hang, Some(T::FallingWater), Some(WATER), false),
            T::FallingWater => (Fluid::Water, DripStage::FallAndLand, Some(T::Splash), Some(WATER), false),
            T::DrippingLava => (Fluid::Lava, DripStage::CoolingHang, Some(T::FallingLava), None, false),
            T::FallingLava => (Fluid::Lava, DripStage::FallAndLand, Some(T::LandingLava), Some(LAVA), false),
            T::LandingLava => (Fluid::Lava, DripStage::Land, None, Some(LAVA), false),
            T::DrippingHoney => (Fluid::None, DripStage::Hang, Some(T::FallingHoney), Some([0.622, 0.508, 0.082]), false),
            T::FallingHoney => (Fluid::None, DripStage::FallAndLand, Some(T::LandingHoney), Some([0.582, 0.448, 0.082]), true),
            T::LandingHoney => (Fluid::None, DripStage::Land, None, Some([0.522, 0.408, 0.082]), false),
            T::FallingNectar => (Fluid::None, DripStage::Fall, None, Some([0.92, 0.782, 0.72]), false),
            T::FallingSporeBlossom => (Fluid::None, DripStage::Fall, None, Some([0.32, 0.5, 0.22]), false),
            T::DrippingObsidianTear => (Fluid::None, DripStage::Hang, Some(T::FallingObsidianTear), Some(TEAR), false),
            T::FallingObsidianTear => (Fluid::None, DripStage::FallAndLand, Some(T::LandingObsidianTear), Some(TEAR), false),
            T::LandingObsidianTear => (Fluid::None, DripStage::Land, None, Some(TEAR), false),
            T::DrippingDripstoneWater => (Fluid::Water, DripStage::Hang, Some(T::FallingDripstoneWater), Some(WATER), false),
            T::FallingDripstoneWater => (Fluid::Water, DripStage::FallAndLand, Some(T::Splash), Some(WATER), true),
            T::DrippingDripstoneLava => (Fluid::Lava, DripStage::CoolingHang, Some(T::FallingDripstoneLava), None, false),
            _ => (Fluid::Lava, DripStage::FallAndLand, Some(T::LandingLava), Some(LAVA), true),
        };
        match stage {
            DripStage::Hang | DripStage::CoolingHang => {
                q.gravity *= 0.02;
                q.lifetime = 40;
            }
            DripStage::FallAndLand => q.lifetime = (64.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32,
            DripStage::Land => q.lifetime = (16.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32,
            DripStage::Fall => {}
        }
        match ty {
            T::FallingHoney | T::FallingObsidianTear => q.gravity = 0.01,
            T::DrippingHoney | T::DrippingObsidianTear => {
                q.gravity *= 0.01;
                q.lifetime = 100;
            }
            T::LandingHoney => q.lifetime = (128.0 / (f(self.provider.next_float()) * 0.8 + 0.2)) as i32,
            T::LandingObsidianTear => q.lifetime = (28.0 / (f(self.provider.next_float()) * 0.8 + 0.2)) as i32,
            T::FallingNectar => {
                q.lifetime = (16.0 / (f(self.provider.next_float()) * 0.8 + 0.2)) as i32;
                q.gravity = 0.007;
            }
            T::FallingSporeBlossom => {
                q.lifetime = (64.0 / (r.next_float() * (0.9 - 0.1) + 0.1)) as i32;
                q.gravity = 0.005;
            }
            _ => {}
        }
        if matches!(ty, T::DrippingObsidianTear | T::FallingObsidianTear | T::LandingObsidianTear) {
            q.glow = Glow::Block15;
        }
        if let Some([cr, cg, cb]) = color {
            q.set_color(cr, cg, cb);
        }
        q.behaviour = Behaviour::Drip { fluid, stage, next, sound };
        q
    }

    /// `WaterDropParticle` (rain) and `SplashParticle`.
    pub(super) fn water_drop(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::moving(r, ty, p, DVec3::ZERO);
        q.velocity.x *= f(0.3);
        q.velocity.y = f(r.next_float() * 0.2 + 0.1);
        q.velocity.z *= f(0.3);
        q.set_size(0.01, 0.01);
        q.gravity = 0.06;
        q.lifetime = (8.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32;
        if ty == Type::Splash {
            q.gravity = 0.04;
            if v.y == 0.0 && (v.x != 0.0 || v.z != 0.0) {
                q.velocity = DVec3::new(v.x, 0.1, v.z);
            }
        }
        q.behaviour = Behaviour::WaterDrop;
        q
    }

    pub(super) fn bubble(r: &mut JavaRandom, ty: Type, p: DVec3, v: DVec3) -> Particle {
        let mut q = Particle::at(r, ty, p);
        if ty == Type::SulfurBubbles {
            q.gravity = -0.04;
            q.friction = 0.85;
            q.set_size(0.02, 0.02);
            q.velocity.x = v.x * f(0.2) + f((r.next_float() * 2.0 - 1.0) * 0.02);
            // The provider passes `yAux` as `za`, so the Z speed comes from Y.
            q.velocity.z = v.y * f(0.2) + f((r.next_float() * 2.0 - 1.0) * 0.02);
            let size_start = 0.02 + 0.02 * r.next_float();
            q.size = size_start;
            q.lifetime = i32::MAX;
            q.behaviour = Behaviour::SulfurBubble { y_start: p.y, y_end: p.y + 4.0 - 1.0, size_start, y_prev: p.y };
            return q;
        }
        q.set_size(0.02, 0.02);
        q.size *= r.next_float() * 0.6 + 0.2;
        q.velocity = DVec3::new(
            v.x * f(0.2) + f((r.next_float() * 2.0 - 1.0) * 0.02),
            v.y * f(0.2) + f((r.next_float() * 2.0 - 1.0) * 0.02),
            v.z * f(0.2) + f((r.next_float() * 2.0 - 1.0) * 0.02),
        );
        if ty == Type::Bubble {
            q.lifetime = (8.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32;
            q.behaviour = Behaviour::Bubble;
        } else {
            q.gravity = -0.125;
            q.friction = 0.85;
            q.lifetime = (40.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32;
            q.behaviour = Behaviour::BubbleColumnUp;
        }
        q
    }

    pub(super) fn current_down(r: &mut JavaRandom, p: DVec3) -> Particle {
        let mut q = Particle::at(r, Type::CurrentDown, p);
        q.lifetime = (r.next_float() * 60.0) as i32 + 30;
        q.has_physics = false;
        q.velocity = DVec3::new(0.0, -0.05, 0.0);
        q.set_size(0.02, 0.02);
        q.size *= r.next_float() * 0.6 + 0.2;
        q.gravity = 0.002;
        q.behaviour = Behaviour::CurrentDown { angle: 0.0 };
        q
    }

    /// `SuspendedParticle`: the providers draw the motion, then the sprite.
    pub(super) fn suspended(&mut self, r: &mut JavaRandom, ty: Type, p: DVec3) -> Particle {
        let below = p - DVec3::new(0.0, 0.125, 0.0);
        let pr = &mut self.provider;
        let vel = match ty {
            Type::CrimsonSpore => DVec3::new(pr.next_gaussian() * f(1.0e-6), pr.next_gaussian() * f(1.0e-4), pr.next_gaussian() * f(1.0e-6)),
            Type::WarpedSpore => DVec3::new(0.0, f(pr.next_float()) * -1.9 * f(pr.next_float()) * 0.1, 0.0),
            _ => DVec3::new(0.0, f(-0.8), 0.0),
        };
        let sprite = self.sprite_from_set(ty);
        let mut q = if ty == Type::Underwater {
            let mut q = Particle::at(r, ty, below);
            q.set_size(0.01, 0.01);
            q.size *= r.next_float() * 0.6 + 0.2;
            q
        } else {
            let mut q = Particle::moving(r, ty, below, vel);
            q.set_size(0.01, 0.01);
            q.size *= r.next_float() * 0.6 + 0.6;
            q
        };
        q.lifetime = (16.0 / (f(r.next_float()) * 0.8 + 0.2)) as i32;
        q.has_physics = false;
        q.friction = 1.0;
        q.gravity = 0.0;
        match ty {
            Type::Underwater => q.set_color(0.4, 0.4, 0.7),
            Type::CrimsonSpore => q.set_color(0.9, 0.4, 0.5),
            Type::WarpedSpore => {
                q.set_color(0.1, 0.1, 0.3);
                q.set_size(0.001, 0.001);
            }
            _ => {
                q.lifetime = self.provider.next_int(501) + 500;
                q.gravity = 0.01;
                q.set_color(0.32, 0.5, 0.22);
            }
        }
        q.sprite = sprite;
        q
    }

    /// The `Options` of a particle made from a plain type: what most requests carry.
    pub(super) fn simple(&mut self, ty: Type, p: DVec3, v: DVec3) {
        self.add(&Options::simple(ty), p, v);
    }
}
