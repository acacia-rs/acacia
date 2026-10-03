//! A synthetic world on the real registry and chunk storage, shared by search and physics tests.

use std::sync::Arc;

use acacia_physics::BlockPos;
use acacia_world::{BlockAccess, BlockIds, BlockRegistry, ChunkView, World};

use crate::pathfind::Terrain;
use crate::world::PhysicsWorld;

/// Chunks -R..=R on both axes are loaded (x, z in -32..48 for R = 2).
const R: i32 = 2;
/// Sub-chunk version 8 with no storages: an empty (all air) section.
const EMPTY_SECTION: &[u8] = &[8, 0];

pub struct Grid {
    pub view: ChunkView,
    pub registry: Arc<BlockRegistry>,
}

impl Grid {
    pub fn new() -> Self {
        let registry = BlockRegistry::vanilla_arc();
        let mut view = ChunkView::new(World::new(registry.clone(), 0, BlockIds::Runtime));
        for cx in -R..=R {
            for cz in -R..=R {
                view.insert_sub_chunk(cx, 0, cz, EMPTY_SECTION).expect("empty section");
            }
        }
        Self { view, registry }
    }

    /// Stone floor at y = 0 over x, z in `lo..=hi`, feet at y = 1.
    pub fn floor(lo: i32, hi: i32) -> Self {
        let g = Self::new();
        g.fill([lo, 0, lo], [hi, 0, hi], "stone");
        g
    }

    /// Runtime id of `minecraft:<name>` whose properties include every `k=v` in `props`.
    pub fn id(&self, name: &str, props: &str) -> u32 {
        let full = format!("minecraft:{name}");
        let want: Vec<&str> = props.split(',').filter(|s| !s.is_empty()).collect();
        let mut states = self.registry.states_of(&full);
        let found = states.find(|(_, s)| want.iter().all(|kv| s.properties.split(',').any(|p| p == *kv)));
        match found {
            Some((id, _)) => id,
            None => {
                let all: Vec<_> = self.registry.states_of(&full).map(|(_, s)| s.properties).take(8).collect();
                panic!("no state {full} [{props}]; some states: {all:?}")
            }
        }
    }

    pub fn set_state(&self, [x, y, z]: BlockPos, name: &str, props: &str) -> &Self {
        let id = if name == "air" { self.registry.air_id() } else { self.id(name, props) };
        assert!(self.view.set_block(x, y, z, 0, id), "outside the loaded grid");
        self
    }

    pub fn fill(&self, min: BlockPos, max: BlockPos, name: &str) -> &Self {
        self.fill_state(min, max, name, "")
    }

    pub fn fill_state(&self, min: BlockPos, max: BlockPos, name: &str, props: &str) -> &Self {
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    self.set_state([x, y, z], name, props);
                }
            }
        }
        self
    }

    /// Flips `open_bit` of the door, gate or trapdoor at `pos` (and of a door's upper half), as
    /// the server does on a click.
    pub fn toggle(&self, [x, y, z]: BlockPos) {
        for pos in [[x, y, z], [x, y + 1, z]] {
            let state = self.registry.get(self.view.block(pos[0], pos[1], pos[2])).expect("known block");
            let Some(open) = state.property("open_bit") else { continue };
            let flipped = state.properties.replace(&format!("open_bit={open}"), &format!("open_bit={}", if open == "1" { 0 } else { 1 }));
            let id = self.registry.find(state.name, &flipped).expect("toggled state");
            self.view.set_block(pos[0], pos[1], pos[2], 0, id);
        }
    }

    pub fn terrain(&self) -> Terrain<'_, &ChunkView> {
        Terrain::new(&self.view, &self.registry)
    }

    pub fn physics(&self) -> PhysicsWorld<'_> {
        PhysicsWorld { view: &self.view, registry: &self.registry }
    }
}
