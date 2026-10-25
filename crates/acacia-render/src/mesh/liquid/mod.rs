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

use super::shapes::NO_AO;
use super::{Ctx, SectionMesh, Volume, model};
use crate::biome::BiomeColors;
use crate::blocks::{BlockTable, Layer, RenderBlock};

/// The faces of the liquid at section-local `p` (none where there is no liquid).
pub fn java_faces(volume: &Volume, table: &BlockTable, biomes: &BiomeColors, p: [i32; 3]) -> Vec<LiquidFace> {
    tesselate(&Ctx { v: volume, table, biomes }, p)
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
    /// Section-local cell whose light it takes, as the brighter of that cell and the one above it
    /// per channel (`FluidRenderer.getLightColor`).
    pub light: [i32; 3],
}

impl LiquidFace {
    /// `BlockAndTintGetter.getShade` for the face, the sides also times the up face's; the Nether
    /// lights up and down alike.
    pub fn shade(&self, nether: bool) -> f32 {
        let up = if nether { 0.9 } else { 1.0 };
        match self.face {
            2 => up,
            3 if nether => 0.9,
            3 => 0.5,
            4 | 5 => up * 0.8,
            _ => up * 0.6,
        }
    }

    /// The vertex colour Java writes (`ARGB.scaleRGB`).
    pub fn color(&self, nether: bool) -> [u8; 3] {
        let shade = self.shade(nether);
        self.tint.map(|c| (f32::from(c) * shade) as u8)
    }
}

pub(super) fn liquid(ctx: &Ctx, p: [i32; 3], out: &mut SectionMesh) {
    let b = fluid_at(ctx, p);
    for face in tesselate(ctx, p) {
        let surface = ctx.surface(p, b, face.sprite.slot());
        let scale = face.sprite.texels(b);
        let corners = face.corners.map(|c| c.map(|v| v * 16.0));
        let uv = face.uv.map(|t| t.map(|v| v * scale));
        let records = model::records4(corners, uv, face.face as u8, surface, NO_AO);
        if b.layer == Layer::Translucent { out.translucent.extend(records) } else { out.solid.extend(records) }
    }
}
