//! Water and lava, as Java 26.3's `FluidRenderer.tesselate` draws them (after Horizon's port,
//! `horizon-client/src/mesher/fluid.rs`, MIT). [`tesselate`] gives Java's faces as floats, which
//! the parity probe compares with the game's; [`liquid`] packs them as four-corner model faces.
//!
//! A liquid block's texture slots: still in up (2), flowing in the sides (4), water's overlay in
//! west (1); a pack without an overlay repeats the flowing texture there.

mod flow;
mod tesselate;
#[cfg(test)]
mod tests;

use tesselate::{faces as tesselate, fluid_at};

use super::quad::vertex_light;
use super::shapes::NO_AO;
use super::smooth::{DOWN, LightWorld, OF_FACE, UP, cardinal};
use super::{Ctx, SectionMesh, Volume, model};
use crate::biome::BiomeColors;
use crate::blocks::{BlockTable, Layer, RenderBlock};

/// The faces of the liquid at section-local `p` (none where there is no liquid).
pub fn java_faces(volume: &Volume, table: &BlockTable, biomes: &BiomeColors, p: [i32; 3]) -> Vec<LiquidFace> {
    tesselate(&Ctx { v: volume, table, biomes, cells: None }, p)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sprite {
    Still,
    Flow,
    Overlay,
}

impl Sprite {
    fn slot(self) -> usize {
        match self {
            Sprite::Still => 2,
            Sprite::Flow => 4,
            Sprite::Overlay => 1,
        }
    }

    /// Texels per sprite unit in the texture layer: a flowing sprite is 32 wide, of which the
    /// layer holds the top-left quarter (`assets::image::Texture::load`'s `quad`).
    fn texels(self, b: &RenderBlock) -> f32 {
        match self {
            Sprite::Still => 16.0,
            Sprite::Overlay if b.textures[1] != b.textures[4] => 16.0,
            _ => 32.0,
        }
    }
}

/// One face as Java emits it.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquidFace {
    /// Section-local block coordinates, in Java's vertex order (counter-clockwise from the front).
    pub corners: [[f32; 3]; 4],
    /// In sprite units, 0..1 across the sprite.
    pub uv: [[f32; 2]; 4],
    pub sprite: Sprite,
    /// The axis face it is shaded as ([`crate::assets::FACE_NAMES`] order); back faces keep their
    /// front's.
    pub face: usize,
    /// sRGB before shading: the biome's water, white for lava.
    pub tint: [u8; 3],
    /// Section-local cell whose light it takes ([`light_coords`]).
    pub light: [i32; 3],
}

impl LiquidFace {
    /// The `CardinalLighting` shade Java scales the face's colour by: the sides' times the up face's.
    pub fn shade(&self, nether: bool) -> f32 {
        let d = OF_FACE[self.face];
        if d == UP || d == DOWN { cardinal(nether, d) } else { cardinal(nether, UP) * cardinal(nether, d) }
    }

    /// The vertex colour Java writes (`ARGB.scaleRGB`).
    pub fn color(&self, nether: bool) -> [u8; 3] {
        let shade = self.shade(nether);
        self.tint.map(|c| (f32::from(c) * shade) as u8)
    }
}

/// `FluidRenderer.getLightCoords` of `face`, every vertex's: the brighter of its light cell and the
/// one above, per channel, as Java's light coords (block << 4 | sky << 20).
pub fn light_coords(world: &impl LightWorld, face: &LiquidFace) -> u32 {
    let coords = |p: [i32; 3]| -> (u32, u32) {
        let facts = world.facts(p);
        if facts.emissive {
            return (240, 240);
        }
        let (block, sky) = world.light(p);
        (u32::from(block.max(facts.emission)) << 4, u32::from(sky) << 4)
    };
    let [x, y, z] = face.light;
    let ((b0, s0), (b1, s1)) = (coords([x, y, z]), coords([x, y + 1, z]));
    b0.max(b1) | s0.max(s1) << 16
}

pub(super) fn liquid(ctx: &Ctx, p: [i32; 3], out: &mut SectionMesh) {
    let b = fluid_at(ctx, p);
    let lighter = ctx.lighter();
    for face in tesselate(ctx, p) {
        let surface = ctx.surface(p, b, face.sprite.slot());
        let scale = face.sprite.texels(b);
        let corners = face.corners.map(|c| c.map(|v| v * 16.0));
        let uv = face.uv.map(|t| t.map(|v| v * scale));
        let mut records = model::records4(corners, uv, face.face as u8, surface, NO_AO);
        if let Some(l) = &lighter {
            // Java bakes the shade into the vertex colour; here it is the vertex grey.
            let (coords, grey) = (light_coords(&l.world, &face), (255.0 * face.shade(ctx.v.nether)) as u32);
            records[0] = records[0].lit([vertex_light(coords & 0xFF, coords >> 16 & 0xFF, grey); 4]);
        }
        if b.layer == Layer::Translucent { out.translucent.extend(records) } else { out.solid.extend(records) }
    }
}
