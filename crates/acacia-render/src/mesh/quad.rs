//! Packed quad format read by `gpu/terrain.wgsl` (vertex pulling, 12 bytes per quad).
//!
//! - w0: x | y << 9 | z << 18 | face << 27 (section-local position in 1/16 block)
//! - w1: width - 1 | height - 1 << 8 | quarter turns << 16 | texture layer << 18 | tint kind << 30
//!   (size in 1/16 block)
//! - w2: ao0..ao3 (2 bits each, 3 = unoccluded) | mirror << 8 | material << 9 | tint rgb (7 bits each) << 11
//!
//! The quarter turns and the mirror are the face's [`Surface::turn`].
//!
//! Liquid faces ([`Quad::liquid`]) span a whole block, so they store other things: w1's size bits
//! hold the surface height at the block's four corners (4 bits each, 1/15 block), w2's AO bits
//! the turn of the surface texture.
//!
//! Bit 31 of w0 marks a model face, which takes three records laid out differently (`model.rs`).

use super::model::{self, MODEL};
use crate::blocks::Material;

/// Face index: 0..6 are the axis faces in [`crate::assets::FACE_NAMES`] order, 6..10 the two
/// diagonal planes of [`crate::blocks::Shape::Cross`], each with a front and a back, and
/// [`LIQUID`].. the axis faces of a liquid.
pub type Face = u8;

pub const LIQUID: Face = 10;
/// A liquid corner height that reaches the top of the block.
pub const FULL_HEIGHT: u8 = 15;

pub const DIRS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
/// (normal axis, u axis, v axis) per axis face; quads span `width` along u and `height` along v.
pub const AXES: [(usize, usize, usize); 6] = [(0, 2, 1), (0, 2, 1), (1, 0, 2), (1, 0, 2), (2, 0, 1), (2, 0, 1)];

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Quad(pub [u32; 3]);

/// What a face looks like, independent of where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
    pub texture: u16,
    /// [`crate::blocks::Tint::shader_kind`].
    pub tint_kind: u32,
    pub material: Material,
    /// sRGB tint, quantized to 7 bits per channel ([`quantize`]).
    pub color: [u8; 3],
    /// How an axis face's texture lies ([`turned`]); model faces carry their own coordinates.
    pub turn: u8,
}

/// Drops each channel to 7 bits, as stored in the quad; merging compares quantized colours.
pub fn quantize(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| v >> 1)
}

/// Where a turned face reads its texture, from the texel `[u, v]` (0..16) its position gives it:
/// mirrored in u when bit 2 of `turn` is set, then turned `turn & 3` quarters about the centre.
pub fn turned(turn: u8, [u, v]: [f32; 2]) -> [f32; 2] {
    let mirrored = [if turn & 4 != 0 { 16.0 - u } else { u }, v];
    (0..turn & 3).fold(mirrored, |[u, v], _| [v, 16.0 - u])
}

#[test]
fn a_quad_holds_its_size_and_how_its_texture_lies() {
    assert_eq!(turned(0, [1.0, 2.0]), [1.0, 2.0]);
    assert_eq!(turned(1, [1.0, 2.0]), [2.0, 15.0]);
    assert_eq!(turned(4, [1.0, 2.0]), [15.0, 2.0]);
    assert_eq!(turned(5, [1.0, 2.0]), [2.0, 1.0]);
    let surface = Surface { texture: 5, tint_kind: 0, material: Material::Opaque, color: [0; 3], turn: 0b110 };
    let [_, w1, w2] = Quad::new([0, 16, 32], 2, [256, 16], surface, [3; 4]).0;
    assert_eq!((w1 & 255, w1 >> 8 & 255, w1 >> 16 & 3, w1 >> 18 & 4095), (255, 15, 2, 5));
    assert_eq!(w2 >> 8 & 1, 1);
}

impl Quad {
    /// `ao` per corner in u/v order (0,0), (1,0), (1,1), (0,1); `size` is 1..=256.
    pub fn new(pos: [u32; 3], face: Face, size: [u32; 2], surface: Surface, ao: [u8; 4]) -> Quad {
        debug_assert!(pos.iter().all(|&v| v < 512) && size.iter().all(|s| (1..=256).contains(s)), "{pos:?} {size:?}");
        let ao_bits = ao.iter().enumerate().fold(0, |a, (i, &v)| a | (u32::from(v) << (i * 2)));
        let [r, g, b] = surface.color.map(u32::from);
        let turn = u32::from(surface.turn);
        Quad([
            pos[0] | pos[1] << 9 | pos[2] << 18 | u32::from(face) << 27,
            (size[0] - 1) | (size[1] - 1) << 8 | (turn & 3) << 16 | u32::from(surface.texture) << 18 | surface.tint_kind << 30,
            ao_bits | (turn >> 2 & 1) << 8 | (surface.material as u32) << 9 | r << 11 | g << 18 | b << 25,
        ])
    }

    /// Face `face` (from [`LIQUID`]) of a liquid block. `pos` is the face's corner on the block's
    /// floor, `heights` the surface at the block's corners (x0z0, x1z0, x1z1, x0z1), `turn` 0 for
    /// an unturned texture, else 1 + the angle in 1/255 turn.
    pub fn liquid(pos: [u32; 3], face: Face, heights: [u8; 4], turn: u8, surface: Surface) -> Quad {
        debug_assert!(face >= LIQUID && heights.iter().all(|&h| h <= FULL_HEIGHT));
        let heights = heights.iter().enumerate().fold(0, |a, (i, &h)| a | (u32::from(h) << (i * 4)));
        let [r, g, b] = surface.color.map(u32::from);
        Quad([
            pos[0] | pos[1] << 9 | pos[2] << 18 | u32::from(face) << 27,
            heights | u32::from(surface.texture) << 18 | surface.tint_kind << 30,
            u32::from(turn) | (surface.material as u32) << 9 | r << 11 | g << 18 | b << 25,
        ])
    }

    /// The axis face or cross plane this quad faces like: liquid faces map to their axis face.
    pub fn face(&self) -> u32 {
        if self.0[0] & MODEL != 0 {
            return self.0[0] & 15;
        }
        let face = (self.0[0] >> 27) & 15;
        if face >= u32::from(LIQUID) { face - u32::from(LIQUID) } else { face }
    }

    /// Sort key for a section's translucent quads: per face direction, far plane first as seen
    /// from the side the face points to. Back faces are culled, so every direction that draws
    /// blends back to front; quads of different directions still blend in key order.
    pub fn blend_order(&self) -> (u32, u32) {
        let face = self.face();
        let Some(&(axis, ..)) = AXES.get(face as usize) else { return (face, 0) };
        let plane = if self.0[0] & MODEL != 0 { model::plane(self, axis) } else { (self.0[0] >> (axis * 9)) & 511 };
        (face, if face.is_multiple_of(2) { plane } else { 511 - plane })
    }
}
