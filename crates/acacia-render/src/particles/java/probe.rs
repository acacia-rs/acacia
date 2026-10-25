//! What a parity probe dumps: a block's effects or one particle, tick by tick, from seeded
//! randoms, as plain records to set against Java's.

use glam::{DVec3, IVec3};

use super::{Engine, JavaRandom, Options, Particle};
use crate::particles::collide::Blocks;

/// One particle's state after a tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
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
    pub removed: bool,
}

impl Snapshot {
    pub fn of(p: &Particle) -> Snapshot {
        Snapshot {
            ty: p.ty.name(),
            position: p.position,
            velocity: p.velocity,
            age: p.age,
            lifetime: p.lifetime,
            sprite: p.sprite,
            size: p.quad_size(0.0),
            rgba: p.rgba,
            on_ground: p.on_ground,
            removed: p.removed,
        }
    }
}

/// The block at `cell` animating once a tick for `ticks` ticks: its `animateTick` (and a lit
/// campfire's block-entity smoke) drawing from `JavaRandom::new(seed)` for the animate random
/// and `seed + 1` for the level's; particles get their own seeds from `seed + 2`, providers draw
/// from `seed + 3`. Returns every live particle after each tick.
pub fn block(blocks: &impl Blocks, cell: IVec3, seed: i64, ticks: u32) -> Vec<Vec<Snapshot>> {
    let mut engine = Engine::seeded(seed);
    engine.animate = JavaRandom::new(seed);
    engine.level = JavaRandom::new(seed.wrapping_add(1));
    engine.seeds = JavaRandom::new(seed.wrapping_add(2));
    engine.provider = JavaRandom::new(seed.wrapping_add(3));
    engine.camera = cell.as_dvec3() + DVec3::splat(0.5);
    (0..ticks)
        .map(|_| {
            engine.campfires_tick(blocks, cell);
            engine.animate_cell(blocks, cell);
            engine.tick(blocks);
            engine.particles.iter().map(Snapshot::of).collect()
        })
        .collect()
}

/// One particle from its provider with its own random seeded `seed` (the provider's draws from
/// `seed + 1`), then `ticks` of its `tick`: the made state first, then one record per tick until
/// it is removed. What it spawns meanwhile is not followed.
pub fn particle(blocks: &impl Blocks, o: &Options, at: DVec3, velocity: DVec3, seed: i64, ticks: u32) -> Vec<Snapshot> {
    let mut engine = Engine::seeded(seed);
    engine.provider = JavaRandom::new(seed.wrapping_add(1));
    engine.requested = Some(Vec::new());
    let Some(mut p) = engine.probe_create(o, at, velocity, seed, blocks) else { return Vec::new() };
    let mut out = vec![Snapshot::of(&p)];
    for _ in 0..ticks {
        if p.removed {
            break;
        }
        engine.probe_tick(&mut p, blocks);
        out.push(Snapshot::of(&p));
    }
    out
}
