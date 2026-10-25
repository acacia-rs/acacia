//! `DripParticle.tick`: hang, fall, land, each stage making the next.

use glam::{DVec3, IVec3};

use super::Engine;
use super::behaviour::DripStage;
use super::level::{self, Fluid};
use super::particle::Particle;
use super::types::Type;
use crate::particles::collide::Blocks;

impl Engine {
    pub(super) fn drip_tick(&mut self, p: &mut Particle, blocks: &impl Blocks, fluid: Fluid, stage: DripStage, next: Option<Type>, sound: bool) {
        p.previous = p.position;
        if stage == DripStage::CoolingHang {
            p.rgba[0] = 1.0;
            p.rgba[1] = 16.0 / (40 - p.lifetime + 16) as f32;
            p.rgba[2] = 4.0 / (40 - p.lifetime + 8) as f32;
        }
        let lifetime = p.lifetime;
        p.lifetime -= 1;
        if lifetime <= 0 {
            p.removed = true;
            if let (DripStage::Hang | DripStage::CoolingHang, Some(next)) = (stage, next) {
                self.simple(next, p.position, p.velocity);
            }
            return;
        }
        p.velocity.y -= f64::from(p.gravity);
        p.move_by(blocks, p.velocity);
        match stage {
            DripStage::Hang | DripStage::CoolingHang => p.velocity *= 0.02,
            DripStage::Fall if p.on_ground => p.removed = true,
            DripStage::FallAndLand if p.on_ground => {
                p.removed = true;
                if let Some(next) = next {
                    self.simple(next, p.position, DVec3::ZERO);
                }
                if sound {
                    // The landing sound's volume.
                    p.random.float();
                }
            }
            _ => {}
        }
        if p.removed {
            return;
        }
        p.velocity *= f64::from(0.98f32);
        if fluid != Fluid::None {
            let b: IVec3 = p.position.floor().as_ivec3();
            let (here, height) = level::fluid_height(blocks, b);
            if here == fluid && p.position.y < f64::from(b.y) + height {
                p.removed = true;
            }
        }
    }
}
