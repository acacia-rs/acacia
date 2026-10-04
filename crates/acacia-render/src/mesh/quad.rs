//! Packed quad format read by `gpu/terrain.wgsl` (vertex pulling, 12 bytes per quad).
//!
//! - w0: x | y << 9 | z << 18 | face << 27 (section-local position in 1/16 block)
//! - w1: width | height << 9 | texture layer << 18 | tint kind << 30 (size in 1/16 block)
//! - w2: ao0..ao3 (2 bits each, 3 = unoccluded) | flip << 8 | material << 9 | tint rgb (7 bits each) << 11

use crate::blocks::Material;

/// Face index: 0..6 are the axis faces in [`crate::assets::FACE_NAMES`] order, 6..10 the two
/// diagonal planes of [`crate::blocks::Shape::Cross`], each with a front and a back.
pub type Face = u8;

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
}

/// Drops each channel to 7 bits, as stored in the quad; merging compares quantized colours.
pub fn quantize(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| v >> 1)
}

impl Quad {
    /// `ao` per corner in u/v order (0,0), (1,0), (1,1), (0,1).
    pub fn new(pos: [u32; 3], face: Face, size: [u32; 2], surface: Surface, ao: [u8; 4]) -> Quad {
        debug_assert!(pos.iter().chain(&size).all(|&v| v < 512), "{pos:?} {size:?}");
        // Split along the brighter diagonal so AO gradients don't crease.
        let flip = (ao[0] + ao[2]) < (ao[1] + ao[3]);
        let ao_bits = ao.iter().enumerate().fold(0, |a, (i, &v)| a | (u32::from(v) << (i * 2)));
        let [r, g, b] = surface.color.map(u32::from);
        Quad([
            pos[0] | pos[1] << 9 | pos[2] << 18 | u32::from(face) << 27,
            size[0] | size[1] << 9 | u32::from(surface.texture) << 18 | surface.tint_kind << 30,
            ao_bits | u32::from(flip) << 8 | (surface.material as u32) << 9 | r << 11 | g << 18 | b << 25,
        ])
    }

    /// Sort key for a section's translucent quads: per face direction, far plane first as seen
    /// from the side the face points to. Back faces are culled, so every direction that draws
    /// blends back to front; quads of different directions still blend in key order.
    pub fn blend_order(&self) -> (u32, u32) {
        let face = self.0[0] >> 27;
        let Some(&(axis, ..)) = AXES.get(face as usize) else { return (face, 0) };
        let plane = (self.0[0] >> (axis * 9)) & 511;
        (face, if face.is_multiple_of(2) { plane } else { 511 - plane })
    }
}
