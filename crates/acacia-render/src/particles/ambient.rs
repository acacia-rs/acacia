//! What blocks around the player give off unprompted: Java's `ClientLevel.animateTick` (26.3)
//! samples 667 cells within 16 blocks and 667 within 32 each tick and runs each one's
//! `animateTick`, then its fluid's, then fluid drips through the block below. Both looks use
//! these rules; Bedrock's own are in the client, not the pack. Campfires smoke from their block
//! entity's tick instead ([`Campfires`]).

use std::collections::HashSet;

use acacia_world::BlockState;
use glam::{DVec3, IVec3, Vec3};

use super::collide::Blocks;
use super::kind::Kind;
use super::rng::Rng;

pub type Out = Vec<(Kind, DVec3, Vec3)>;

const SAMPLES: u32 = 667;
const RADII: [i32; 2] = [16, 32];

pub fn animate(blocks: &impl Blocks, centre: IVec3, rng: &mut Rng, campfires: &mut Campfires, out: &mut Out) {
    for _ in 0..SAMPLES {
        for r in RADII {
            let mut axis = || rng.below(r as u32) as i32 - rng.below(r as u32) as i32;
            let cell = centre + IVec3::new(axis(), axis(), axis());
            let Some(state) = blocks.block(cell) else { continue };
            block(blocks, state, cell, rng, out);
            if is_campfire(state) {
                campfires.found.insert(cell);
            }
            let fluid = if state.is_liquid() { Some(state) } else { blocks.liquid(cell) };
            if let Some(fluid) = fluid {
                if fluid.is_lava() && blocks.block(cell + IVec3::Y).is_some_and(BlockState::is_air) && rng.below(100) == 0 {
                    out.push((Kind::Lava, at(cell, rng.f32(), 1.0, rng.f32()), Vec3::ZERO));
                }
                if rng.below(10) == 0 {
                    drip(blocks, state, cell, fluid.is_lava(), rng, out);
                }
            }
        }
    }
}

fn at(cell: IVec3, x: f32, y: f32, z: f32) -> DVec3 {
    cell.as_dvec3() + Vec3::new(x, y, z).as_dvec3()
}

/// A Bedrock horizontal direction's step.
fn step(direction: &str) -> Option<IVec3> {
    Some(match direction {
        "north" => IVec3::NEG_Z,
        "south" => IVec3::Z,
        "west" => IVec3::NEG_X,
        "east" => IVec3::X,
        _ => return None,
    })
}

fn lit(state: &BlockState) -> bool {
    state.property("lit") == Some("1")
}

fn block(blocks: &impl Blocks, state: &BlockState, cell: IVec3, rng: &mut Rng, out: &mut Out) {
    let name = state.name.strip_prefix("minecraft:").unwrap_or(state.name);
    match name {
        "torch" | "soul_torch" | "copper_torch" => {
            // Bedrock's facing is the side the torch leans to, the opposite of Java's wall_torch.
            let wick = match state.property("torch_facing_direction").and_then(step) {
                Some(s) => at(cell, 0.5 + 0.27 * s.x as f32, 0.92, 0.5 + 0.27 * s.z as f32),
                None => at(cell, 0.5, 0.7, 0.5),
            };
            let flame = if name == "soul_torch" { Kind::SoulFlame } else { Kind::Flame };
            out.extend([(Kind::Smoke, wick, Vec3::ZERO), (flame, wick, Vec3::ZERO)]);
        }
        "redstone_torch" => {
            let mut jitter = || (rng.f32() - 0.5) * 0.2;
            let (x, y, z) = (jitter(), jitter(), jitter());
            let wall = state.property("torch_facing_direction").and_then(step).map_or(Vec3::ZERO, |s| Vec3::new(0.27 * s.x as f32, 0.22, 0.27 * s.z as f32));
            out.push((Kind::REDSTONE, at(cell, 0.5 + x + wall.x, 0.7 + y + wall.y, 0.5 + z + wall.z), Vec3::ZERO));
        }
        "campfire" if state.property("extinguished") == Some("0") => {
            if rng.below(10) == 0 {
                rng.f32();
                rng.f32();
            }
            if rng.below(5) == 0 {
                out.push((Kind::Lava, at(cell, 0.5, 0.5, 0.5), Vec3::new(rng.f32() / 2.0, 5.0e-5, rng.f32() / 2.0)));
            }
        }
        "lit_furnace" | "lit_blast_furnace" => {
            let _crackle = rng.f32() < 0.1;
            let front = state.property("minecraft:cardinal_direction").and_then(step).unwrap_or(IVec3::Z);
            let t = rng.f32() * 0.6 - 0.3;
            let dx = if front.x != 0 { front.x as f32 * 0.52 } else { t };
            let dy = rng.f32() * if name == "lit_furnace" { 6.0 } else { 9.0 } / 16.0;
            let dz = if front.z != 0 { front.z as f32 * 0.52 } else { t };
            let p = at(cell, 0.5 + dx, dy, 0.5 + dz);
            out.push((Kind::Smoke, p, Vec3::ZERO));
            if name == "lit_furnace" {
                out.push((Kind::Flame, p, Vec3::ZERO));
            }
        }
        "lit_smoker" => out.push((Kind::Smoke, at(cell, 0.5, 1.1, 0.5), Vec3::ZERO)),
        n if (n == "candle" || n.ends_with("_candle")) && lit(state) => {
            let count = state.property("candles").and_then(|c| c.parse::<usize>().ok()).unwrap_or(0).min(3);
            for &[x, y, z] in CANDLE_WICKS[count] {
                candle(at(cell, x / 16.0, y / 16.0, z / 16.0), rng, out);
            }
        }
        n if n.ends_with("candle_cake") && lit(state) => candle(at(cell, 0.5, 1.0, 0.5), rng, out),
        "fire" | "soul_fire" => {
            if rng.below(24) == 0 {
                rng.f32();
                rng.f32();
            }
            // Java also smokes along burnable sides; only fire on a floor is drawn.
            if blocks.block(cell - IVec3::Y).is_some_and(BlockState::is_full_cube) {
                for _ in 0..3 {
                    out.push((Kind::LargeSmoke, at(cell, rng.f32(), rng.f32() * 0.5 + 0.5, rng.f32()), Vec3::ZERO));
                }
            }
        }
        "lit_redstone_ore" | "lit_deepslate_redstone_ore" => {
            for side in [IVec3::NEG_Y, IVec3::Y, IVec3::NEG_Z, IVec3::Z, IVec3::NEG_X, IVec3::X] {
                if blocks.block(cell + side).is_some_and(BlockState::is_full_cube) {
                    continue;
                }
                let mut coord = |s: i32| if s != 0 { 0.5 + 0.5625 * s as f32 } else { rng.f32() };
                let (x, y, z) = (coord(side.x), coord(side.y), coord(side.z));
                out.push((Kind::REDSTONE, at(cell, x, y, z), Vec3::ZERO));
            }
        }
        _ => {}
    }
}

/// `AbstractCandleBlock`'s wicks in sixteenths, by candle count.
const CANDLE_WICKS: [&[[f32; 3]]; 4] = [
    &[[8.0, 8.0, 8.0]],
    &[[6.0, 7.0, 8.0], [10.0, 8.0, 7.0]],
    &[[8.0, 5.0, 10.0], [6.0, 7.0, 8.0], [9.0, 8.0, 7.0]],
    &[[7.0, 5.0, 9.0], [10.0, 7.0, 9.0], [6.0, 7.0, 6.0], [9.0, 8.0, 6.0]],
];

fn candle(wick: DVec3, rng: &mut Rng, out: &mut Out) {
    if rng.f32() < 0.3 {
        out.push((Kind::Smoke, wick, Vec3::ZERO));
    }
    out.push((Kind::SmallFlame, wick, Vec3::ZERO));
}

/// Java's `trySpawnDripParticles`: a drop under the block below a fluid, or under a ceiling it
/// soaks through.
fn drip(blocks: &impl Blocks, state: &BlockState, cell: IVec3, lava: bool, rng: &mut Rng, out: &mut Out) {
    let below = cell - IVec3::Y;
    let Some(under) = blocks.block(below) else { return };
    if under.is_liquid() || blocks.liquid(below).is_some() {
        return;
    }
    let kind = if lava { Kind::DrippingLava } else { Kind::DrippingWater };
    let (lo, hi) = extent(under);
    let mut spawn = |lo: [f32; 3], hi: [f32; 3], y: f32| {
        let (x, z) = (lo[0] + rng.f32() * (hi[0] - lo[0]), lo[2] + rng.f32() * (hi[2] - lo[2]));
        out.push((kind, at(below, x, y, z), Vec3::ZERO));
    };
    if hi[1] < 1.0 {
        // Only a block with a sturdy bottom holds the fluid it drips from.
        if state.is_full_cube() {
            spawn([0.0; 3], [1.0; 3], 1.0 - 0.05);
        }
        return;
    }
    if under.name.contains("glass") || under.name.ends_with("_leaves") {
        return;
    }
    if lo[1] > 0.0 {
        spawn(lo, hi, lo[1] - 0.05);
    } else if blocks.block(below - IVec3::Y).is_some_and(|s| extent(s).1[1] < 1.0 && !s.is_liquid()) && blocks.liquid(below - IVec3::Y).is_none() {
        spawn(lo, hi, -0.05);
    }
}

/// The bounds of a block's collision boxes; an empty shape's top is below everything.
fn extent(state: &BlockState) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for b in state.boxes {
        for i in 0..3 {
            lo[i] = lo[i].min(b.min[i]);
            hi[i] = hi[i].max(b.max[i]);
        }
    }
    (lo, hi)
}

fn is_campfire(state: &BlockState) -> bool {
    matches!(state.name, "minecraft:campfire" | "minecraft:soul_campfire") && state.property("extinguished") == Some("0")
}

/// Lit campfires the sampling came across, smoking each tick as `CampfireBlockEntity.particleTick`
/// does while they stay lit and within reach.
#[derive(Default)]
pub struct Campfires {
    found: HashSet<IVec3>,
}

/// Campfires further than this from the player are forgotten.
const CAMPFIRE_REACH: i32 = 48;

impl Campfires {
    pub fn tick(&mut self, blocks: &impl Blocks, centre: IVec3, rng: &mut Rng, out: &mut Out) {
        self.found.retain(|&c| (c - centre).abs().max_element() <= CAMPFIRE_REACH && blocks.block(c).is_some_and(is_campfire));
        for &cell in &self.found {
            if rng.f32() >= 0.11 {
                continue;
            }
            // A hay bale below makes it a signal fire.
            let signal = blocks.block(cell - IVec3::Y).is_some_and(|s| s.name == "minecraft:hay_block");
            let kind = if signal { Kind::SignalSmoke } else { Kind::CampfireSmoke };
            for _ in 0..rng.below(2) + 2 {
                let x = 0.5 + rng.f32() / 3.0 * rng.sign();
                let y = rng.f32() + rng.f32();
                let z = 0.5 + rng.f32() / 3.0 * rng.sign();
                out.push((kind, at(cell, x, y, z), Vec3::Y * 0.07));
            }
        }
    }
}
