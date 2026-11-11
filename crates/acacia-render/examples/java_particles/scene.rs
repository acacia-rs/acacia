//! A case's blocks as Bedrock states (the harness maps the oracle's Java states); air elsewhere.

use std::collections::HashMap;

use acacia_render::particles::Blocks;
use acacia_world::{BlockRegistry, BlockState};
use glam::IVec3;
use serde_json::Value;

pub struct Scene {
    registry: &'static BlockRegistry,
    placed: HashMap<IVec3, u32>,
    air: u32,
}

impl Scene {
    /// From `[[x, y, z, "minecraft:name[key=value,...]"], ...]`.
    pub fn new(blocks: &Value) -> Scene {
        let registry = BlockRegistry::vanilla();
        let id = |key: &str| {
            let (name, properties) = key.split_once('[').map_or((key, ""), |(n, p)| (n, p.trim_end_matches(']')));
            registry.find(name, properties).unwrap_or_else(|| panic!("no Bedrock state {key}"))
        };
        let placed = blocks
            .as_array()
            .expect("blocks")
            .iter()
            .map(|b| (IVec3::new(coord(&b[0]), coord(&b[1]), coord(&b[2])), id(b[3].as_str().expect("state"))))
            .collect();
        Scene { registry, placed, air: id("minecraft:air") }
    }
}

fn coord(v: &Value) -> i32 {
    v.as_i64().expect("coordinate") as i32
}

impl Blocks for Scene {
    fn block(&self, cell: IVec3) -> Option<&BlockState> {
        self.registry.get(*self.placed.get(&cell).unwrap_or(&self.air))
    }

    fn liquid(&self, _: IVec3) -> Option<&BlockState> {
        None
    }
}
