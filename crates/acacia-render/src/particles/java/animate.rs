//! `ClientLevel.animateTick`: 667 cells within 16 blocks and 667 within 32 each tick, picked
//! with the level's random, each running its block's and fluid's `animateTick` with the
//! animate random, and fluid drips through the block below. Lit campfires also smoke from
//! their block entity's tick, which this keeps for the campfires the sampling has met.

use glam::{DVec3, IVec3};

use super::{Engine, JavaRandom};
use super::behaviour::Options;
use super::classify::{Animate, classify};
use super::level::{self, Fluid};
use super::types::Type;
use crate::particles::collide::Blocks;

const SAMPLES: u32 = 667;

/// `random.nextDouble() / 3.0 * (random.nextBoolean() ? 1 : -1)`.
fn offset(r: &mut JavaRandom) -> f64 {
    r.next_double() / 3.0 * if r.next_boolean() { 1.0 } else { -1.0 }
}

/// Campfires further than this from the player are forgotten.
const CAMPFIRE_REACH: i32 = 48;

impl Engine {
    /// One client tick's ambient effects around `centre`, the player's block.
    pub fn animate_tick(&mut self, blocks: &impl Blocks, centre: IVec3) {
        self.campfires_tick(blocks, centre);
        for _ in 0..SAMPLES {
            for radius in [16, 32] {
                let r = &mut self.level;
                let mut axis = || r.next_int(radius) - r.next_int(radius);
                let cell = centre + IVec3::new(axis(), axis(), axis());
                self.animate_cell(blocks, cell);
            }
        }
    }

    /// `doAnimateTick` at one cell, without the draws that pick it.
    pub fn animate_cell(&mut self, blocks: &impl Blocks, cell: IVec3) {
        let Some(state) = blocks.block(cell) else { return };
        let kind = classify(state);
        if let Animate::Campfire { .. } = kind {
            self.campfires.insert(cell);
        }
        if kind != Animate::None {
            self.block_animate(blocks, kind, cell);
        }
        let fluid = level::fluid(blocks, cell);
        if fluid.fluid == Fluid::None {
            return;
        }
        self.fluid_animate(blocks, cell, fluid);
        if self.level.next_int(10) == 0 {
            let drip = if fluid.fluid == Fluid::Water { Type::DrippingWater } else { Type::DrippingLava };
            self.try_drip(blocks, cell - IVec3::Y, drip, level::sturdy(blocks, cell));
        }
    }

    /// `WaterFluid.animateTick` and `LavaFluid.animateTick`.
    fn fluid_animate(&mut self, blocks: &impl Blocks, cell: IVec3, fluid: level::FluidState) {
        let c = cell.as_dvec3();
        let r = &mut self.animate;
        if fluid.fluid == Fluid::Water {
            if !fluid.is_source() && !fluid.falling {
                if r.next_int(64) == 0 {
                    self.sound(2);
                }
            } else if r.next_int(10) == 0 {
                let p = c + DVec3::new(r.next_double(), r.next_double(), r.next_double());
                self.simple(Type::Underwater, p, DVec3::ZERO);
            }
            return;
        }
        let above = cell + IVec3::Y;
        if !level::is_air(blocks, above) || level::solid_render(blocks, above) {
            return;
        }
        if r.next_int(100) == 0 {
            let p = c + DVec3::new(r.next_double(), 1.0, r.next_double());
            self.simple(Type::Lava, p, DVec3::ZERO);
            self.sound(2);
        }
        if self.animate.next_int(200) == 0 {
            self.sound(2);
        }
    }

    /// `ClientLevel.trySpawnDripParticles` under a fluid, from the level's random.
    fn try_drip(&mut self, blocks: &impl Blocks, cell: IVec3, drip: Type, top_solid: bool) {
        if level::fluid(blocks, cell).fluid != Fluid::None {
            return;
        }
        let Some(shape) = level::extent(blocks, cell) else { return };
        let c = cell.as_dvec3();
        if shape.top < 1.0 {
            if top_solid {
                self.drip_at([c.x, c.x + 1.0], [c.z, c.z + 1.0], c.y + 1.0 - 0.05, drip);
            }
            return;
        }
        if level::impermeable(blocks, cell) {
            return;
        }
        let (x, z) = ([c.x + shape.x[0], c.x + shape.x[1]], [c.z + shape.z[0], c.z + shape.z[1]]);
        if shape.bottom > 0.0 {
            self.drip_at(x, z, c.y + shape.bottom - 0.05, drip);
            return;
        }
        let below = cell - IVec3::Y;
        let below_top = level::extent(blocks, below).map_or(1.0, |b| b.top);
        if below_top < 1.0 && level::fluid(blocks, below).fluid == Fluid::None {
            self.drip_at(x, z, c.y - 0.05, drip);
        }
    }

    /// `spawnFluidParticle` between world-space bounds.
    fn drip_at(&mut self, x: [f64; 2], z: [f64; 2], y: f64, drip: Type) {
        let r = &mut self.level;
        let (a, b) = (r.next_double(), r.next_double());
        self.simple(drip, DVec3::new(level::lerp(a, x[0], x[1]), y, level::lerp(b, z[0], z[1])), DVec3::ZERO);
    }

    /// `CampfireBlockEntity.particleTick` for each lit campfire still near, without cooking items.
    pub fn campfires_tick(&mut self, blocks: &impl Blocks, centre: IVec3) {
        let lit = |cell: IVec3| blocks.block(cell).is_some_and(|s| matches!(classify(s), Animate::Campfire { .. }));
        self.campfires.retain(|&c| (c - centre).abs().max_element() <= CAMPFIRE_REACH && lit(c));
        let mut cells: Vec<IVec3> = self.campfires.iter().copied().collect();
        cells.sort_unstable_by_key(|c| (c.x, c.y, c.z));
        for cell in cells {
            self.campfire_tick(blocks, cell);
        }
    }

    /// One campfire's `particleTick`; a hay bale below makes it a signal fire, as Bedrock has
    /// no `signal_fire` property.
    pub fn campfire_tick(&mut self, blocks: &impl Blocks, cell: IVec3) {
        if self.level.next_float() >= 0.11 {
            return;
        }
        let signal = blocks.block(cell - IVec3::Y).is_some_and(|s| s.name == "minecraft:hay_block");
        let ty = if signal { Type::CampfireSignalSmoke } else { Type::CampfireCosySmoke };
        let mut i = 0;
        while i < self.level.next_int(2) + 2 {
            let (r, c) = (&mut self.level, cell.as_dvec3());
            let x = c.x + 0.5 + offset(r);
            let y = c.y + r.next_double() + r.next_double();
            let z = c.z + 0.5 + offset(r);
            self.add_always(&Options::simple(ty), DVec3::new(x, y, z), DVec3::new(0.0, 0.07, 0.0));
            i += 1;
        }
    }
}
