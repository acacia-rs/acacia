//! The renderer's particles: spawns from the caller, game ticks against the shown world's blocks,
//! and both particle passes (block chips, sprites).

use std::cell::RefCell;
use std::sync::Arc;

use acacia_world::{BlockState, Chunk, SharedChunk, World};
use glam::{DVec3, IVec3, Vec3};

use super::Renderer;
use super::particles::ParticlePass;
use super::sprites::SpritePass;
use crate::particles::{Blocks, Kind, Particles, Sheet};

/// Ticks caught up at most after a stall (a second).
const MAX_CATCH_UP: u64 = 20;

pub struct ParticleFeed {
    chips: ParticlePass,
    sprites: SpritePass,
    sheet: Option<Sheet>,
    particles: Particles,
    /// The game tick the particles were last advanced to.
    tick: u64,
}

impl ParticleFeed {
    pub fn new(device: &wgpu::Device, color: wgpu::TextureFormat, globals: &wgpu::Buffer) -> Self {
        ParticleFeed { chips: ParticlePass::new(device, color), sprites: SpritePass::new(device, color, globals), sheet: None, particles: Particles::default(), tick: 0 }
    }

    /// `textures` is the block texture array the chips are cut from.
    pub fn draw(&self, device: &wgpu::Device, pass: &mut wgpu::RenderPass<'_>, globals: &wgpu::Buffer, textures: &wgpu::TextureView, sampler: &wgpu::Sampler) {
        self.chips.draw(device, pass, globals, textures, sampler);
        self.sprites.draw(pass);
    }
}

impl Renderer {
    /// The chips of the block broken at `pos`; `runtime` is its id in the shown world.
    pub fn break_particles(&mut self, pos: IVec3, runtime: u32) {
        let Some(scene) = &self.scene else { return };
        let layer = scene.table().get(runtime).textures[0];
        self.particles.particles.break_block(pos, layer);
    }

    /// `count` particles (or bursts) within `spread` blocks of `at`; `velocity` as Java's
    /// `addParticle` takes it.
    pub fn spawn_particles(&mut self, kind: Kind, at: DVec3, velocity: Vec3, count: u32, spread: f32) {
        self.particles.particles.scatter(kind, at, velocity, count, spread);
    }

    /// The look's particle sprites ([`Sheet::load`]); they also decide whose particles spawn.
    pub fn set_particle_sheet(&mut self, sheet: Sheet) {
        let feed = &mut self.particles;
        feed.sprites.set_sheet(&self.device, &self.queue, &sheet);
        feed.particles.style = sheet.style;
        feed.particles.sprites.clear();
        feed.sheet = Some(sheet);
    }

    /// Advances the particles to the game tick now, with the blocks around the camera giving
    /// off theirs, then uploads them.
    pub(super) fn prepare_particles(&mut self, camera: &crate::Camera) {
        let ticks = self.started.elapsed().as_secs_f64() * 20.0;
        let now = ticks as u64;
        let feed = &mut self.particles;
        if let Some(scene) = &self.scene {
            let blocks = WorldBlocks::new(scene.world());
            let centre = camera.position.floor().as_ivec3();
            for _ in 0..now.saturating_sub(feed.tick).min(MAX_CATCH_UP) {
                feed.particles.tick(&blocks, Some(centre));
            }
        }
        feed.tick = now;
        let partial = ticks.fract();
        let light = self.scene.as_ref().map(|s| s.light().clone());
        let light = light.as_ref().map(|l| l.read());
        // Outside lit columns a particle is as bright as open sky.
        let light_at = |p: DVec3| {
            let c = p.floor().as_ivec3();
            let byte = light.as_ref().and_then(|l| l.light(c.x, c.y, c.z)).unwrap_or(15);
            [f32::from(byte >> 4), f32::from(byte & 15)]
        };
        let (forward, right) = (camera.forward(), camera.right());
        let up = right.cross(forward);
        feed.chips.prepare(&self.device, &self.queue, &feed.particles.chips, partial, camera.position, right, up, light_at);
        if let Some(sheet) = &feed.sheet {
            feed.sprites.prepare(&self.device, &self.queue, sheet, &feed.particles.sprites, partial, camera.position, right, up, light_at);
        }
    }
}

type ChunkAt = ((i32, i32), Arc<SharedChunk>);

/// The shown world's blocks, remembering the chunk read last (particles ask about neighbours).
struct WorldBlocks<'a> {
    world: &'a World,
    last: RefCell<Option<ChunkAt>>,
}

impl<'a> WorldBlocks<'a> {
    fn new(world: &'a World) -> Self {
        WorldBlocks { world, last: RefCell::new(None) }
    }

    fn id(&self, cell: IVec3, layer: impl Fn(&Chunk) -> u32) -> Option<u32> {
        let key = (cell.x >> 4, cell.z >> 4);
        let mut last = self.last.borrow_mut();
        if last.as_ref().is_none_or(|(k, _)| *k != key) {
            *last = Some((key, self.world.get(key.0, key.1)?));
        }
        let (_, chunk) = last.as_ref()?;
        Some(layer(&chunk.read()))
    }
}

impl Blocks for WorldBlocks<'_> {
    fn block(&self, cell: IVec3) -> Option<&BlockState> {
        self.world.registry().get(self.id(cell, |c| c.block(cell.x, cell.y, cell.z))?)
    }

    fn liquid(&self, cell: IVec3) -> Option<&BlockState> {
        self.world.registry().get(self.id(cell, |c| c.liquid(cell.x, cell.y, cell.z))?).filter(|s| s.is_liquid())
    }
}
