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

use crate::block::{model_faces, overlaid, render_block};
use crate::blockstate::{self, Drawn, ModelRef};
use crate::mapping::{JavaState, Mapping};
use crate::model::Models;
use crate::model::bake::{BakedFace, bake};
use crate::textures::Textures;
use crate::{fluid, light, occlusion, offset, shade, tint};

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
/// `shapes` gives the look Java's face culling; without them it keeps the renderer's own.
pub fn bake_look(pack: &Pack, assets: &Path, mapping: &Mapping, tags: &fluid::Tags, shapes: Option<&occlusion::Shapes>) -> (LookPack, Report) {
    let (mut look, _) = LookPack::bake_bedrock(pack, Look::JAVA);
    let registry = BlockRegistry::vanilla();
    let mut baker = Baker { models: Models::new(assets.to_owned()), textures: Textures::new(assets.to_owned()) };
    let mut blockstates: HashMap<String, Option<Value>> = HashMap::new();
    let mut report = Report::default();
    let mut numbers = occlusion::BlockNumbers::default();
    for id in 0..registry.len() as u32 {
        let state = registry.get(id).expect("id below len");
        let key = state_key(state);
        let base = look.block(&key).expect("the Bedrock bake covers the registry").clone();
        if base.fluid != Fluid::None {
            match liquid(&base, &mut baker.textures, &mut look.atlas) {
                Some(mut block) => {
                    report.liquids += 1;
                    if let Some(java) = mapping.get(&key) {
                        block.light = light::facts(java, state, &block, |shapes| look.light_shape(shapes));
                        block.cull = shapes.map(|s| occlusion::cull(s, java, &mut numbers, |faces| look.light_shape(faces)));
                    }
                    look.set_block(key, block);
                }
                None => *report.kept.entry("texture missing").or_default() += 1,
            }
            continue;
        }
        let mut faces = None;
        let block = match () {
            _ if blocks::model::classify(state).is_some() => Err("drawn as a block entity"),
            _ => mapping.get(&key).ok_or("not in the mapping").and_then(|java| {
                let file = blockstates.entry(java.name.clone()).or_insert_with(|| {
                    serde_json::from_slice(&std::fs::read(assets.join(format!("blockstates/{}.json", java.name))).ok()?).ok()
                });
                let drawn = blockstate::drawn(file.as_ref().ok_or("no blockstate file")?, java);
                // Before baking: a state Java draws nothing for (a bare wall) covers nothing.
                faces = Some(baker.first_faces(&drawn));
                baker.block(&base, &drawn, java, state.light_emission > 0, &mut look.atlas)
            }),
        };
        let mut block = match block {
            Ok(block) => {
                if block.shape == Shape::Cube { report.cubes += 1 } else { report.models += 1 }
                block
            }
            Err(why) => {
                *report.kept.entry(why).or_default() += 1;
                base
            }
        };
        // Whatever draws it, a block darkens the faces around it and takes light as Java has it.
        if let Some(java) = mapping.get(&key) {
            block.shades = shade::darkens(java, state.is_full_cube());
            block.fluid_rules = fluid::rules(block.fluid_rules, java, faces.as_deref(), tags);
            block.light = light::facts(java, state, &block, |shapes| look.light_shape(shapes));
            block.cull = shapes.map(|s| occlusion::cull(s, java, &mut numbers, |faces| look.light_shape(faces)));
        }
        look.set_block(key, block);
    }
    look.compact();
    report.missing_textures = baker.textures.missing.into_iter().collect();
    report.invalid_models = baker.models.invalid;
    (look, report)
}

/// A liquid takes Java's textures in the slots the liquid mesher reads (`mesh/liquid`): still on
/// top and bottom, flowing on the sides, and water's overlay in the west slot.
fn liquid(base: &RenderBlock, textures: &mut Textures, atlas: &mut Atlas) -> Option<RenderBlock> {
    let name = if base.fluid == Fluid::Water { "water" } else { "lava" };
    let (still, _) = textures.layer(&format!("block/{name}_still"), atlas)?;
    let (flow, _) = textures.layer(&format!("block/{name}_flow"), atlas)?;
    let overlay = if base.fluid == Fluid::Water { textures.layer("block/water_overlay", atlas).map_or(flow, |(l, _)| l) } else { flow };
    Some(RenderBlock { textures: [flow, overlay, still, still, flow, flow], ..base.clone() })
}

struct Baker {
    models: Models,
    textures: Textures,
}

impl Baker {
    fn faces(&mut self, model: &ModelRef) -> Vec<BakedFace> {
        bake(&self.models.resolve(&model.model), model)
    }

    /// The faces of the first alternative of every part: what the block is to its neighbours.
    fn first_faces(&mut self, drawn: &Drawn) -> Vec<BakedFace> {
        drawn.first().cloned().collect::<Vec<_>>().iter().flat_map(|m| self.faces(m)).collect()
    }

    /// What `java` draws as, or why it keeps `base`, its Bedrock rendering. `gives_light` is of the block.
    fn block(&mut self, base: &RenderBlock, drawn: &Drawn, java: &JavaState, gives_light: bool, atlas: &mut Atlas) -> Result<RenderBlock, &'static str> {
        let first = self.first_faces(drawn);
        if first.is_empty() {
            return Err("no Java geometry");
        }
        // ModelBlockRenderer.tesselateBlock: the first part's model decides, and never for a light.
        let ambient_occlusion = !gives_light && first[0].ambient_occlusion;
        let tint = |index| tint::of(java, index);
        let offset = offset::of(&java.name);
        let mut whole = |baker: &mut Baker, faces: &[BakedFace], cube: bool| -> Result<RenderBlock, &'static str> {
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
