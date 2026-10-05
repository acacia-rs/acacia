//! The Java look: the Bedrock look with every block the Java assets model drawn their way. Blocks
//! the Java assets leave to code keep their Bedrock rendering (chests, signs) or only change
//! textures (liquids).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use acacia_render::assets::Pack;
use acacia_render::assets::flipbook::Atlas;
use acacia_render::blocks::placed::{Random, Weighted};
use acacia_render::blocks::{self, Fluid, RenderBlock, Shape};
use acacia_render::lookpack::state_key;
use acacia_render::{Look, LookPack};
use acacia_world::BlockRegistry;
use serde_json::Value;

use crate::block::{model_faces, overlaid, render_block, side_turns};
use crate::blockstate::{self, Drawn, ModelRef};
use crate::mapping::{JavaState, Mapping};
use crate::model::Models;
use crate::model::bake::{BakedFace, bake};
use crate::textures::Textures;
use crate::{offset, tint};

/// What became of the vanilla registry's states.
#[derive(Default)]
pub struct Report {
    /// Drawn as greedy-meshed cubes and as model faces.
    pub cubes: usize,
    pub models: usize,
    /// Liquids, drawn the renderer's way with Java's textures.
    pub liquids: usize,
    /// States left with their Bedrock rendering, by why.
    pub kept: BTreeMap<&'static str, usize>,
    pub missing_textures: Vec<String>,
    pub invalid_models: Vec<String>,
}

/// `pack` is the Bedrock resource pack, `assets` the jar's `assets/minecraft`.
pub fn bake_look(pack: &Pack, assets: &Path, mapping: &Mapping) -> (LookPack, Report) {
    let (mut look, _) = LookPack::bake_bedrock(pack, Look::JAVA);
    let registry = BlockRegistry::vanilla();
    let mut baker = Baker { models: Models::new(assets.to_owned()), textures: Textures::new(assets.to_owned()), dry_foliage: tint::dry_foliage(assets) };
    let mut blockstates: HashMap<String, Option<Value>> = HashMap::new();
    let mut report = Report::default();
    for id in 0..registry.len() as u32 {
        let state = registry.get(id).expect("id below len");
        let key = state_key(state);
        let base = look.block(&key).expect("the Bedrock bake covers the registry").clone();
        if base.fluid != Fluid::None {
            match liquid(&base, &mut baker.textures, &mut look.atlas) {
                Some(block) => {
                    report.liquids += 1;
                    look.set_block(key, block);
                }
                None => *report.kept.entry("texture missing").or_default() += 1,
            }
            continue;
        }
        let block = match () {
            _ if blocks::model::classify(state).is_some() => Err("drawn as a block entity"),
            _ => mapping.get(&key).ok_or("not in the mapping").and_then(|java| {
                let file = blockstates.entry(java.name.clone()).or_insert_with(|| {
                    serde_json::from_slice(&std::fs::read(assets.join(format!("blockstates/{}.json", java.name))).ok()?).ok()
                });
                let drawn = blockstate::drawn(file.as_ref().ok_or("no blockstate file")?, java);
                baker.block(&base, &drawn, java, state.light_emission > 0, &mut look.atlas)
            }),
        };
        match block {
            Ok(block) => {
                if block.shape == Shape::Cube { report.cubes += 1 } else { report.models += 1 }
                look.set_block(key, block);
            }
            Err(why) => *report.kept.entry(why).or_default() += 1,
        }
    }
    look.compact();
    report.missing_textures = baker.textures.missing.into_iter().collect();
    report.invalid_models = baker.models.invalid;
    (look, report)
}

/// A liquid keeps the renderer's liquid geometry and takes Java's textures: still on top and
/// bottom, flowing on the sides.
fn liquid(base: &RenderBlock, textures: &mut Textures, atlas: &mut Atlas) -> Option<RenderBlock> {
    let name = if base.fluid == Fluid::Water { "water" } else { "lava" };
    let (still, _) = textures.layer(&format!("block/{name}_still"), atlas)?;
    let (flow, _) = textures.layer(&format!("block/{name}_flow"), atlas)?;
    Some(RenderBlock { textures: [flow, flow, still, still, flow, flow], ..base.clone() })
}

struct Baker {
    models: Models,
    textures: Textures,
    dry_foliage: [u8; 3],
}

impl Baker {
    fn faces(&mut self, model: &ModelRef) -> Vec<BakedFace> {
        bake(&self.models.resolve(&model.model), model)
    }

    /// What `java` draws as, or why it keeps `base`, its Bedrock rendering. `gives_light` is of the block.
    fn block(&mut self, base: &RenderBlock, drawn: &Drawn, java: &JavaState, gives_light: bool, atlas: &mut Atlas) -> Result<RenderBlock, &'static str> {
        let first: Vec<BakedFace> = drawn.first().cloned().collect::<Vec<_>>().iter().flat_map(|m| self.faces(m)).collect();
        if first.is_empty() {
            return Err("no Java geometry");
        }
        // ModelBlockRenderer.tesselateBlock: the first part's model decides, and never for a light.
        let ambient_occlusion = !gives_light && first[0].ambient_occlusion;
        let dry_foliage = self.dry_foliage;
        let tint = |index| tint::of(java, index, dry_foliage);
        let offset = offset::of(&java.name);
        let mut whole = |baker: &mut Baker, faces: &[BakedFace], cube: bool| -> Result<RenderBlock, &'static str> {
            if overlaid(faces) {
                // Bedrock's cube draws the overlay; only how the faces lie is Java's.
                let turns = side_turns(faces).filter(|_| base.shape == Shape::Cube).ok_or("overlaid faces")?;
                return Ok(RenderBlock { turns, ..base.clone() });
            }
            let model = model_faces(faces, &tint, ambient_occlusion, &mut baker.textures, atlas).ok_or("texture missing")?;
            Ok(RenderBlock { offset, ..render_block(base, faces, model, cube) })
        };
        if drawn.parts.iter().all(|part| part.len() == 1) {
            return whole(self, &first, true);
        }
        if !drawn.multipart {
            let alternatives: Vec<(u32, Vec<BakedFace>)> = drawn.parts[0].iter().map(|(weight, model)| (*weight, self.faces(model))).collect();
            let mut blocks = |baker: &mut Baker, cube: bool| -> Result<Vec<(u32, RenderBlock)>, &'static str> {
                alternatives.iter().map(|(weight, faces)| Ok((*weight, whole(baker, faces, cube)?))).collect()
            };
            let mut baked = blocks(self, true)?;
            // The mesher takes cubes and models down different paths; one block goes down one.
            if baked.iter().any(|(_, b)| b.shape != baked[0].1.shape) {
                baked = blocks(self, false)?;
            }
            let first = baked[0].1.clone();
            return Ok(RenderBlock { random: Some(Arc::new(Random::Whole(Weighted(baked.into())))), ..first });
        }
        if overlaid(&first) {
            return Err("overlaid faces");
        }
        let (mut fixed, mut random) = (Vec::new(), Vec::new());
        for part in &drawn.parts {
            let mut alternatives = Vec::with_capacity(part.len());
            for (weight, model) in part {
                let faces = self.faces(model);
                let faces = model_faces(&faces, &tint, ambient_occlusion, &mut self.textures, atlas).ok_or("texture missing")?;
                alternatives.push((*weight, faces));
            }
            match alternatives.len() {
                1 => fixed.extend(alternatives.pop().expect("one alternative").1),
                _ => random.push(Weighted(alternatives.into_iter().map(|(weight, faces)| (weight, Arc::from(faces))).collect())),
            }
        }
        // What the block is to its neighbours comes from the first alternatives.
        let model = model_faces(&first, &tint, ambient_occlusion, &mut self.textures, atlas).ok_or("texture missing")?;
        let block = render_block(base, &first, model, false);
        Ok(RenderBlock { shape: Shape::Model(fixed.into()), random: Some(Arc::new(Random::Parts(random.into()))), offset, ..block })
    }
}
