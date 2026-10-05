//! A look pack: the baked blocks, textures and parameters the renderer draws with, whatever edition
//! they came from. Design: docs/java-look.md.

mod compact;
mod files;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acacia_world::{BlockRegistry, BlockState};
use rustc_hash::FxHashMap;

use crate::assets::Pack;
use crate::assets::flipbook::Atlas;
use crate::blocks::{self, BlockTable, BuildReport, RenderBlock};
use crate::look::Look;

#[derive(Clone)]
pub struct LookPack {
    pub look: Look,
    /// Distinct blocks; `states` indexes into it.
    blocks: Vec<RenderBlock>,
    /// By [`state_key`].
    states: FxHashMap<String, u32>,
    /// Blocks set since the pack was made, by their JSON: equal blocks share an entry.
    distinct: FxHashMap<String, u32>,
    /// The texture array the blocks' layers index.
    pub atlas: Atlas,
    /// See [`LookPack::files`].
    files: PathBuf,
}

/// A block state's name in a pack: `minecraft:oak_log[pillar_axis=y]`, or the bare name.
pub fn state_key(state: &BlockState) -> String {
    if state.properties.is_empty() { state.name.to_owned() } else { format!("{}[{}]", state.name, state.properties) }
}

impl LookPack {
    /// `$ACACIA_LOOKS`, else `assets/looks` under the working directory, then the look's name.
    pub fn default_dir(name: &str) -> PathBuf {
        std::env::var_os("ACACIA_LOOKS").map_or_else(|| PathBuf::from("assets/looks"), PathBuf::from).join(name)
    }

    /// Bakes the vanilla registry's blocks from a Bedrock resource pack.
    pub fn bake_bedrock(pack: &Pack, look: Look) -> (LookPack, BuildReport) {
        let registry = BlockRegistry::vanilla();
        let (table, atlas, report) = BlockTable::build(registry, pack);
        let mut baked = LookPack { look, blocks: Vec::new(), states: FxHashMap::default(), distinct: FxHashMap::default(), atlas, files: pack.root().to_owned() };
        for id in 0..registry.len() as u32 {
            let state = registry.get(id).expect("id below len");
            baked.set_block(state_key(state), table.get(id).clone());
        }
        (baked, report)
    }

    /// What a state draws as, by [`state_key`].
    pub fn block(&self, state: &str) -> Option<&RenderBlock> {
        self.states.get(state).map(|&index| &self.blocks[index as usize])
    }

    /// Makes a state draw as `block`. The model is dropped: it follows from the state.
    pub fn set_block(&mut self, state: String, block: RenderBlock) {
        let block = RenderBlock { model: None, ..block };
        let json = serde_json::to_string(&block).expect("a render block serializes");
        let index = *self.distinct.entry(json).or_insert_with(|| {
            self.blocks.push(block);
            self.blocks.len() as u32 - 1
        });
        self.states.insert(state, index);
    }

    pub fn with_look(mut self, look: Look) -> LookPack {
        self.look = look;
        self
    }

    /// Where the files the pack does not bake are, laid out as in a Bedrock resource pack: entity
    /// models and textures, colormaps, `biomes_client.json`, sky textures.
    pub fn files(&self) -> &Path {
        &self.files
    }

    /// (states, distinct blocks)
    pub fn counts(&self) -> (usize, usize) {
        (self.states.len(), self.blocks.len())
    }

    /// The table for a world's registry. States the pack lacks (custom blocks) are missing-texture cubes.
    pub fn block_table(&self, registry: &BlockRegistry) -> BlockTable {
        let block = |id| {
            let state = registry.get(id).expect("id below len");
            let Some(&index) = self.states.get(&state_key(state)) else { return BlockTable::cube(0) };
            RenderBlock { model: blocks::model::classify(state).map(Arc::new), ..self.blocks[index as usize].clone() }
        };
        let mut table = BlockTable::from_blocks((0..registry.len() as u32).map(block).collect());
        table.biome_blend = self.look.biome_blend;
        table
    }
}
