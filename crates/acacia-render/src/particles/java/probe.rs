//! What the particles parity probe dumps (tools/parity/harnesses/particles): a block's effects or
//! one particle, tick by tick, from randoms seeded as the oracle seeds Java's.

use glam::{DVec3, IVec3};
use rustc_hash::FxHashMap;

use super::classify::{Animate, classify};
use super::{Engine, JavaRandom, Options, Particle, Request, Type};
use crate::particles::collide::Blocks;

/// The seeds Java's particles got in one tick (recovered from their randoms), by type and position bits.
pub type Seeds = FxHashMap<(Type, [u64; 3]), i64>;

/// One particle's state after a tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub seed: i64,
    /// The Java registry name, e.g. `smoke`.
    pub ty: &'static str,
    pub position: DVec3,
    pub velocity: DVec3,
    pub age: i32,
    pub lifetime: i32,
    pub sprite: Option<&'static str>,
    /// `getQuadSize(0)`.
    pub size: f32,
    pub rgba: [f32; 4],
    pub on_ground: bool,
}

impl Snapshot {
    pub fn of(p: &Particle) -> Snapshot {
        Snapshot {
            seed: p.seed,
            ty: p.ty.name(),
            position: p.position,
            velocity: p.velocity,
            age: p.age,
            lifetime: p.lifetime,
            sprite: p.sprite,
            size: p.quad_size(0.0),
            rgba: p.rgba,
            on_ground: p.on_ground,
        }
    }
}

/// One tick: what was made (`ParticleEngine.createParticle`), then every live drawn particle by seed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tick {
    pub spawns: Vec<Request>,
    pub particles: Vec<Snapshot>,
}

/// The animate random from `seed`, the level's from `seed + 1`, particles without a Java seed from
/// `seed + 2`, the provider's from `seed + 3`.
fn engine(seed: i64, camera: DVec3) -> Engine {
    let mut engine = Engine::seeded(seed);
    engine.animate = JavaRandom::new(seed);
    engine.level = JavaRandom::new(seed.wrapping_add(1));
    engine.seeds = JavaRandom::new(seed.wrapping_add(2));
    engine.provider = JavaRandom::new(seed.wrapping_add(3));
    engine.camera = camera;
    engine.spawned = Some(Vec::new());
    engine
}

fn begin(engine: &mut Engine, seeds: &[Seeds], tick: usize) {
    engine.given = Some(seeds.get(tick).cloned().unwrap_or_default());
}

fn end(engine: &mut Engine) -> Tick {
    let spawns = engine.spawned.replace(Vec::new()).unwrap_or_default();
    let mut particles: Vec<Snapshot> = engine.particles.iter().filter(|p| !p.no_render).map(Snapshot::of).collect();
    particles.sort_by_key(|p| p.seed);
    Tick { spawns, particles }
}

/// The block at `cell` animating once a tick, as `doAnimateTick` once it has picked the cell, after
/// a lit campfire's block-entity tick, then `ParticleEngine.tick`. `foliage` is the biome's colour there.
pub fn block(blocks: &impl Blocks, cell: IVec3, seed: i64, ticks: usize, foliage: u32, seeds: &[Seeds]) -> Vec<Tick> {
    let mut engine = engine(seed, cell.as_dvec3() + DVec3::splat(0.5));
    engine.foliage = foliage;
    if blocks.block(cell).is_some_and(|s| matches!(classify(s), Animate::Campfire { .. })) {
        engine.campfires.insert(cell);
    }
    (0..ticks)
        .map(|t| {
            begin(&mut engine, seeds, t);
            engine.campfires_tick(blocks, cell);
            engine.animate_cell(blocks, cell);
            engine.tick(blocks);
            end(&mut engine)
        })
        .collect()
}

/// One particle made before the first tick, then `ticks` of `ParticleEngine.tick`, following what
/// it spawns.
pub fn particle(blocks: &impl Blocks, o: &Options, at: DVec3, velocity: DVec3, seed: i64, ticks: usize, seeds: &[Seeds]) -> Vec<Tick> {
    let mut engine = engine(seed, at);
    (0..ticks)
        .map(|t| {
            begin(&mut engine, seeds, t);
            if t == 0 {
                engine.create(o, at, velocity);
            }
            engine.tick(blocks);
            end(&mut engine)
        })
        .collect()
}

impl Engine {
    /// A particle as its provider makes it, ticked by its constructor where Java's is.
    pub fn probe_create(&mut self, o: &Options, p: DVec3, v: DVec3, seed: i64, blocks: &impl Blocks) -> Option<Particle> {
        let mut particle = self.make(o, p, v, seed)?;
        self.constructor_tick(&mut particle, blocks);
        Some(particle)
    }

    /// One `Particle.tick`.
    pub fn probe_tick(&mut self, particle: &mut Particle, blocks: &impl Blocks) {
        self.tick_particle(particle, blocks);
    }
}
