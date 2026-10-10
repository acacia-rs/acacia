//! Per-runtime-id render data: shape, render layer, texture-array layer and tint per face.

pub mod model;
pub mod placed;
pub mod shape;
pub mod tint;

use std::sync::Arc;

use acacia_world::{BlockRegistry, BlockState};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::assets::Pack;
use crate::assets::flipbook::{Animation, Atlas};
use crate::assets::image::{Alpha, Texture};
pub use shape::{Box16, ModelFace, Shape, short_name};
pub use tint::Tint;

/// How a face's texture alpha is used; the value is the shader's material index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Material {
    /// Alpha ignored.
    Opaque = 0,
    /// Texels below half alpha are discarded.
    Cutout = 1,
    Blend = 2,
    /// Opaque; alpha is the tint mask (grass block sides).
    Overlay = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    Invisible,
    /// Opaque and cutout, depth-written.
    Solid,
    Translucent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fluid {
    None,
    Water,
    Lava,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderBlock {
    pub shape: Shape,
    pub layer: Layer,
    /// Texture-array layer per face ([`crate::assets::FACE_NAMES`] order).
    pub textures: [u16; 6],
    pub tint: [Tint; 6],
    pub material: [Material; 6],
    /// How each face's texture lies ([`crate::mesh::quad::turned`]).
    pub turns: [u8; 6],
    /// Alternatives chosen by where the block is. Neighbours see this block, whichever is drawn.
    pub random: Option<Arc<placed::Random>>,
    /// Model faces are shifted by it.
    pub offset: Option<placed::Offset>,
    /// Full opaque cube: hides neighbour faces and darkens ambient occlusion.
    pub occludes: bool,
    /// Darkens the ambient occlusion of faces around it. Java's leaves do without occluding.
    pub shades: bool,
    /// Faces against the same block are hidden (glass seams); leaves keep them like fancy leaves.
    pub cull_same: bool,
    pub fluid: Fluid,
    /// Liquid surface height in 1/16 block.
    pub fluid_height: u8,
    /// Drawn as an entity model; the shape is then [`Shape::None`]. Follows from the state, so a
    /// look pack does not store it.
    #[serde(skip)]
    pub model: Option<Arc<model::BlockModel>>,
}

pub struct BlockTable {
    blocks: Vec<RenderBlock>,
    fallback: RenderBlock,
    /// [`crate::look::Look::biome_blend`] of the look the table draws.
    pub biome_blend: u8,
}

/// What [`BlockTable::build`] could not resolve, for diagnostics.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub blocks_without_textures: Vec<&'static str>,
    pub missing_images: Vec<String>,
}

/// Blocks whose textures have alpha even though the block is a solid cube in the game.
const CUTOUT_CUBES: &[&str] = &["leaves", "glass", "spawner", "ice", "roots", "scaffolding", "cobweb", "grate", "vault", "beacon", "chorus"];
const TRANSLUCENT: &[&str] = &["stained_glass", "slime", "honey_block"];

impl BlockTable {
    /// Returns the table and the texture array it indexes.
    pub fn build(registry: &BlockRegistry, pack: &Pack) -> (BlockTable, Atlas, BuildReport) {
        let mut textures = Textures::new(pack);
        let mut report = BuildReport::default();
        let blocks = (0..registry.len() as u32)
            .map(|id| {
                let state = registry.get(id).expect("id below len");
                let faces = face_names(pack, state);
                if faces.is_none() && classify_needs_texture(state) {
                    report.blocks_without_textures.push(state.name);
                }
                build_block(state, faces, &mut textures)
            })
            .collect();
        report.blocks_without_textures.dedup();
        report.missing_images = textures.missing;
        let fallback = BlockTable::cube(0);
        (BlockTable { blocks, fallback, biome_blend: 1 }, textures.atlas, report)
    }

    /// One block per runtime id.
    pub(crate) fn from_blocks(blocks: Vec<RenderBlock>) -> BlockTable {
        BlockTable { blocks, fallback: BlockTable::cube(0), biome_blend: 1 }
    }

    /// Runtime ids past the registry (unknown custom blocks) render as a missing-texture cube.
    pub fn get(&self, id: u32) -> &RenderBlock {
        self.blocks.get(id as usize).unwrap_or(&self.fallback)
    }

    pub(crate) fn cube(texture: u16) -> RenderBlock {
        RenderBlock {
            shape: Shape::Cube,
            layer: Layer::Solid,
            textures: [texture; 6],
            tint: [Tint::None; 6],
            material: [Material::Opaque; 6],
            turns: [0; 6],
            random: None,
            offset: None,
            occludes: true,
            shades: true,
            cull_same: true,
            fluid: Fluid::None,
            fluid_height: 0,
            model: None,
        }
    }
}

fn classify_needs_texture(state: &BlockState) -> bool {
    model::classify(state).is_none() && shape::classify(state) != Shape::None
}

fn face_names(pack: &Pack, state: &BlockState) -> Option<[String; 6]> {
    let name = short_name(state.name);
    // blocks.json lags renames; some blocks only have a terrain texture of their own name.
    let faces = pack
        .block_faces(name)
        .or_else(|| match name {
            "grass_block" => pack.block_faces("grass"),
            "iron_chain" => pack.block_faces("chain"),
            _ => None,
        })
        .or_else(|| pack.texture(name).map(|_| std::array::from_fn(|_| name.to_owned())))?;
    Some(orient(faces, state))
}

/// Applies `pillar_axis` and `minecraft:cardinal_direction` (blocks.json fronts face south).
fn orient(mut f: [String; 6], state: &BlockState) -> [String; 6] {
    let (top, bottom, side) = (f[2].clone(), f[3].clone(), f[4].clone());
    match state.property("pillar_axis") {
        Some("x") => f = [top.clone(), bottom, side.clone(), side.clone(), side.clone(), side],
        Some("z") => f = [side.clone(), side.clone(), side.clone(), side, top.clone(), bottom],
        _ => {}
    }
    let turns = match state.property("minecraft:cardinal_direction") {
        Some("west") => 1,
        Some("north") => 2,
        Some("east") => 3,
        _ => 0,
    };
    // South, west, north, east: each turn moves a face one step along this ring.
    const RING: [usize; 4] = [4, 1, 5, 0];
    let old = f.clone();
    for i in 0..4 {
        f[RING[(i + turns) % 4]] = old[RING[i]].clone();
    }
    f
}

fn build_block(state: &BlockState, faces: Option<[String; 6]>, textures: &mut Textures) -> RenderBlock {
    let name = short_name(state.name);
    let model = model::classify(state).map(Arc::new);
    let shape = if model.is_some() { Shape::None } else { shape::classify(state) };
    let resolved = match faces {
        Some(f) if shape != Shape::None => f.map(|t| textures.resolve(&t)),
        _ => [Resolved::MISSING; 6],
    };
    let fluid = match () {
        _ if state.is_water() => Fluid::Water,
        _ if state.is_lava() => Fluid::Lava,
        _ => Fluid::None,
    };
    let translucent = fluid == Fluid::Water || TRANSLUCENT.iter().any(|t| name.contains(t)) || matches!(name, "ice" | "portal");
    let cutout_cube = CUTOUT_CUBES.iter().any(|c| name.contains(c));
    let layer = match shape {
        Shape::None => Layer::Invisible,
        _ if translucent => Layer::Translucent,
        _ => Layer::Solid,
    };
    let material = resolved.map(|r| match () {
        _ if r.overlay => Material::Overlay,
        _ if layer == Layer::Translucent => Material::Blend,
        _ if shape == Shape::Cube && !cutout_cube => Material::Opaque,
        _ if fluid == Fluid::Lava || r.alpha == Alpha::Opaque => Material::Opaque,
        _ => Material::Cutout,
    });
    let occludes = shape == Shape::Cube && layer == Layer::Solid && material.iter().all(|m| matches!(m, Material::Opaque | Material::Overlay));
    RenderBlock {
        shape,
        layer,
        textures: resolved.map(|r| r.layer),
        tint: tint::faces(name),
        material,
        turns: [0; 6],
        random: None,
        offset: None,
        occludes,
        shades: occludes,
        cull_same: !name.ends_with("leaves"),
        fluid,
        fluid_height: (state.fluid_height() * 16.0).round() as u8,
        model,
    }
}

#[derive(Clone, Copy)]
struct Resolved {
    layer: u16,
    alpha: Alpha,
    overlay: bool,
}

impl Resolved {
    const MISSING: Resolved = Resolved { layer: 0, alpha: Alpha::Opaque, overlay: false };
}

/// Loads each distinct image once into the texture-array layer list.
struct Textures<'a> {
    pack: &'a Pack,
    atlas: Atlas,
    by_name: FxHashMap<String, Resolved>,
    missing: Vec<String>,
}

impl<'a> Textures<'a> {
    fn new(pack: &'a Pack) -> Self {
        let atlas = Atlas { layers: vec![Texture::missing()], animations: Vec::new() };
        Textures { pack, atlas, by_name: FxHashMap::default(), missing: Vec::new() }
    }

    fn resolve(&mut self, texture_name: &str) -> Resolved {
        if let Some(r) = self.by_name.get(texture_name) {
            return *r;
        }
        let r = self.load(texture_name).unwrap_or_else(|| {
            self.missing.push(texture_name.to_owned());
            Resolved::MISSING
        });
        self.by_name.insert(texture_name.to_owned(), r);
        r
    }

    fn load(&mut self, texture_name: &str) -> Option<Resolved> {
        let tex = self.pack.texture(texture_name)?;
        let file = self.pack.image_file(&tex.path)?;
        let layer = self.atlas.layers.len() as u16;
        let warn = |e: &crate::Error| tracing::warn!(%e, "texture");
        let image = match self.pack.flipbook(texture_name) {
            Some(book) => {
                let strip = Texture::load_frames(&file, tex.quad).inspect_err(warn).ok()?;
                let still = strip.first()?.clone();
                let animation = Animation::new(layer, strip, book);
                let first = animation.as_ref().map_or(still, |a| a.at(0));
                self.atlas.animations.extend(animation);
                first
            }
            None => Texture::load(&file, tex.quad).inspect_err(warn).ok()?,
        };
        let r = Resolved { layer, alpha: image.alpha(), overlay: tex.overlay };
        self.atlas.layers.push(image);
        Some(r)
    }
}
