//! The scenes of `java-world-light` (world_light.rs): Bedrock chunks dumped from a movement trace
//! (`acacia-bot` example `trace_region`) rebuilt as a [`World`], and their light against the game's.

use std::sync::Arc;

use acacia_render::light::LightData;
use acacia_world::{BlockIds, BlockRegistry, SharedChunk, World};
use serde::Deserialize;

use super::Outcome;

#[derive(Deserialize)]
struct Section {
    y: i32,
    blocks: Vec<usize>,
    liquid: Option<Vec<usize>>,
}

#[derive(Deserialize)]
struct Chunk {
    x: i32,
    z: i32,
    sections: Vec<Section>,
}

#[derive(Deserialize)]
pub struct Scene {
    pub scene: String,
    dimension: String,
    pub min_y: i32,
    pub height: u32,
    centre: [i32; 2],
    radius: i32,
    palette: Vec<String>,
    chunks: Vec<Chunk>,
}

impl Scene {
    /// The chunks compared: all but the outer ring, whose own neighbours are missing.
    pub fn inner(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let r = self.radius - 1;
        let [x, z] = self.centre;
        (x - r..=x + r).flat_map(move |cx| (z - r..=z + r).map(move |cz| (cx, cz)))
    }

    /// The scene's chunks in a world of their dimension; the chunks are held by the second value.
    pub fn build(&self, registry: &Arc<BlockRegistry>) -> (Arc<World>, Vec<Arc<SharedChunk>>) {
        let id = match self.dimension.as_str() {
            "the_nether" => 1,
            "the_end" => 2,
            _ => 0,
        };
        let world = World::new(registry.clone(), id, BlockIds::Runtime);
        let air = world.dimension().air;
        let ids: Vec<u32> = self
            .palette
            .iter()
            .map(|k| {
                let (name, props) = k.split_once('[').map_or((k.as_str(), ""), |(n, p)| (n, p.trim_end_matches(']')));
                registry.find(name, props).unwrap_or(air)
            })
            .collect();
        let mut held = Vec::new();
        for c in &self.chunks {
            for sy in 0..self.height as i32 / 16 {
                held.push(world.insert_sub_chunk_air(c.x, (self.min_y >> 4) + sy, c.z));
            }
            for s in &c.sections {
                for i in 0..4096 {
                    let (x, y, z) = (c.x * 16 + (i as i32 >> 8), self.min_y + s.y * 16 + (i as i32 & 15), c.z * 16 + (i as i32 >> 4 & 15));
                    if ids[s.blocks[i]] != air {
                        world.set_block(x, y, z, 0, ids[s.blocks[i]]);
                    }
                    if let Some(liquid) = &s.liquid
                        && ids[liquid[i]] != air
                    {
                        world.set_block(x, y, z, 1, ids[liquid[i]]);
                    }
                }
            }
        }
        (world, held)
    }

    /// The inner chunks' light, cell by cell, against the game's (`theirs`, in WorldLightOracle's
    /// light.bin order); returns how many bytes of it the scene took.
    pub fn compare_cells(&self, ours: &LightData, theirs: &[u8], world: &World, outcome: &mut Outcome) -> usize {
        let mut i = 0;
        for (cx, cz) in self.inner() {
            for y in self.min_y..self.min_y + self.height as i32 {
                for z in cz * 16..cz * 16 + 16 {
                    for x in cx * 16..cx * 16 + 16 {
                        let (want, got) = (theirs[i], ours.light(x, y, z).unwrap_or(0));
                        i += 1;
                        outcome.cases += 1;
                        if want != got {
                            let show = |v: u8| format!("block {} sky {}", v >> 4, v & 15);
                            outcome.diverge(&format!("{}: {} at {:?}", self.scene, block_name(world, [x, y, z]), [x, y, z]), "cell light", show(want), show(got));
                        }
                    }
                }
            }
        }
        i
    }
}

pub fn block_name(world: &World, [x, y, z]: [i32; 3]) -> String {
    let block = world.get(x >> 4, z >> 4).map(|c| c.read().block(x, y, z)).unwrap_or(0);
    world.registry().get(block).map_or_else(|| "?".to_owned(), |s| s.name.to_owned())
}
