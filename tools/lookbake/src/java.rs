//! The Java look: the Bedrock look with every block the Java assets model drawn their way. Blocks
//! the Java assets leave to code (liquids, chests, signs) keep their Bedrock rendering.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use acacia_render::assets::Pack;
use acacia_render::assets::flipbook::Atlas;
use acacia_render::assets::image::Alpha;
use acacia_render::blocks::{self, Fluid, Layer, Material, ModelFace, RenderBlock, Shape, Tint};
use acacia_render::lookpack::state_key;
use acacia_render::{Look, LookPack};
use acacia_world::BlockRegistry;
use serde_json::Value;

use crate::blockstate;
use crate::mapping::Mapping;
use crate::model::Models;
use crate::model::bake::{BakedFace, bake};
use crate::textures::Textures;

/// What became of the vanilla registry's states.
#[derive(Default)]
pub struct Report {
    /// Drawn as greedy-meshed cubes and as model faces.
    pub cubes: usize,
    pub models: usize,
    /// States left with their Bedrock rendering, by why.
    pub kept: BTreeMap<&'static str, usize>,
    pub missing_textures: Vec<String>,
    pub invalid_models: Vec<String>,
}

/// `pack` is the Bedrock resource pack, `assets` the jar's `assets/minecraft`.
pub fn bake_look(pack: &Pack, assets: &Path, mapping: &Mapping) -> (LookPack, Report) {
    let (mut look, _) = LookPack::bake_bedrock(pack, Look::JAVA);
    let registry = BlockRegistry::vanilla();
    let (mut models, mut textures) = (Models::new(assets.to_owned()), Textures::new(assets.to_owned()));
    let mut blockstates: HashMap<String, Option<Value>> = HashMap::new();
    let mut report = Report::default();
    for id in 0..registry.len() as u32 {
        let state = registry.get(id).expect("id below len");
        let key = state_key(state);
        let base = look.block(&key).expect("the Bedrock bake covers the registry").clone();
        let faces = match () {
            _ if base.fluid != Fluid::None => Err("liquid"),
            _ if blocks::model::classify(state).is_some() => Err("drawn as a block entity"),
            _ => mapping.get(&key).ok_or("not in the mapping").and_then(|java| {
                let file = blockstates.entry(java.name.clone()).or_insert_with(|| {
                    serde_json::from_slice(&std::fs::read(assets.join(format!("blockstates/{}.json", java.name))).ok()?).ok()
                });
                let placed = blockstate::models(file.as_ref().ok_or("no blockstate file")?, java);
                let faces: Vec<BakedFace> = placed.iter().flat_map(|p| bake(&models.resolve(&p.model), p)).collect();
                match () {
                    _ if faces.is_empty() => Err("no Java geometry"),
                    _ if overlaid(&faces) => Err("overlaid faces"),
                    _ => Ok(faces),
                }
            }),
        };
        let block = faces.and_then(|f| render_block(&base, &f, &mut textures, &mut look.atlas).ok_or("texture missing"));
        match block {
            Ok(block) => {
                if block.shape == Shape::Cube { report.cubes += 1 } else { report.models += 1 }
                look.set_block(key, block);
            }
            Err(why) => *report.kept.entry(why).or_default() += 1,
        }
    }
    look.compact();
    report.missing_textures = textures.missing.into_iter().collect();
    report.invalid_models = models.invalid;
    (look, report)
}

/// Two faces on the same quad facing the same way (grass block sides under their overlay): they
/// would fight for depth. The two sides of a plane share corners but face opposite ways.
fn overlaid(faces: &[BakedFace]) -> bool {
    let corners = |f: &BakedFace| {
        let fixed = f.positions.map(|p| p.map(|v| (v * 64.0).round() as i32));
        let (a, b, c) = (fixed[0], fixed[1], fixed[3]);
        let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let normal = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]].map(i32::signum);
        let mut c = fixed;
        c.sort_unstable();
        (c, normal)
    };
    let mut seen: Vec<_> = faces.iter().map(corners).collect();
    seen.sort_unstable();
    seen.windows(2).any(|pair| pair[0] == pair[1])
}

/// `None` when a face's texture has no image.
fn render_block(base: &RenderBlock, faces: &[BakedFace], textures: &mut Textures, atlas: &mut Atlas) -> Option<RenderBlock> {
    // Which faces tint is the model's to say; with what is by Bedrock block name, as in the Bedrock look.
    let tint = base.tint.iter().copied().find(|t| *t != Tint::None).unwrap_or(Tint::None);
    let mut model = Vec::with_capacity(faces.len());
    for f in faces {
        let (texture, alpha) = textures.layer(&f.texture, atlas)?;
        let material = match alpha {
            Alpha::Opaque => Material::Opaque,
            Alpha::Cutout => Material::Cutout,
            Alpha::Blended => Material::Blend,
        };
        model.push(ModelFace {
            corners: [f.positions[0], f.positions[1], f.positions[3]],
            uv: [f.uvs[0], f.uvs[1], f.uvs[3]],
            texture,
            tint: if f.tinted { tint } else { Tint::None },
            material,
            shade: f.shade.map(|d| d.face()),
            cull: f.cull.map(|d| d.face()),
        });
    }
    let translucent = model.iter().any(|f| f.material == Material::Blend);
    let opaque = model.iter().all(|f| f.material == Material::Opaque);
    let full = (0..6).all(|side| faces.iter().any(|f| f.cull.map(|d| d.face()) == Some(side) && covers_side(f)));
    let mut block = RenderBlock {
        shape: Shape::None,
        layer: if translucent { Layer::Translucent } else { Layer::Solid },
        textures: [0; 6],
        tint: [Tint::None; 6],
        material: [Material::Opaque; 6],
        occludes: full && opaque,
        cull_same: base.cull_same,
        fluid: Fluid::None,
        fluid_height: 0,
        model: None,
    };
    if full && faces.len() == 6 && faces.iter().all(plain) {
        for (f, baked) in model.iter().zip(faces) {
            let side = usize::from(baked.cull.expect("a full block's faces cull").face());
            (block.textures[side], block.tint[side], block.material[side]) = (f.texture, f.tint, f.material);
        }
        block.shape = Shape::Cube;
    } else {
        block.shape = Shape::Model(Arc::from(model));
    }
    Some(block)
}

const EPSILON: f32 = 1e-3;

/// The face fills its side of the block.
fn covers_side(f: &BakedFace) -> bool {
    let on_corner = |v: f32| v.abs() < EPSILON || (v - 16.0).abs() < EPSILON;
    f.positions.iter().all(|p| p.iter().all(|v| on_corner(*v))) && f.shade == f.cull
}

/// Textured as the renderer textures a cube face: by position, unrotated.
fn plain(f: &BakedFace) -> bool {
    let Some(side) = f.cull else { return false };
    f.positions.iter().zip(&f.uvs).all(|(p, uv)| {
        let [u, v] = side.project(*p);
        (u - uv[0]).abs() < EPSILON && (v - uv[1]).abs() < EPSILON
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::bake::Direction;

    fn face(positions: [[f32; 3]; 4]) -> BakedFace {
        let uvs = positions.map(|p| Direction::South.project(p));
        BakedFace { positions, uvs, texture: "block/poppy".into(), cull: None, tinted: false, shade: None }
    }

    #[test]
    fn a_plane_drawn_from_both_sides_is_not_overlaid() {
        let front = face([[0.0, 16.0, 8.0], [0.0, 0.0, 8.0], [16.0, 0.0, 8.0], [16.0, 16.0, 8.0]]);
        let back = face([[16.0, 16.0, 8.0], [16.0, 0.0, 8.0], [0.0, 0.0, 8.0], [0.0, 16.0, 8.0]]);
        assert!(!overlaid(&[front.clone(), back]));
        assert!(overlaid(&[front.clone(), front]));
    }
}
