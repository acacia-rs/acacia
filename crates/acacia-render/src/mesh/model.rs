//! Model faces in the quad buffer: three records per face, read by `gpu/model.wgsl`. Corners 0, 1
//! and 3 of the parallelogram are stored; the shader finds corner 2.
//!
//! - record 0: w0 = [`MODEL`] | kind | ao << 4 | material << 12 | tint kind << 14 | layer << 16
//!   | flip << 28 (`ao` and `flip` as in a plain quad's w2),
//!   w1 = c0.x | c0.y << 16, w2 = c0.z | c1.x << 16
//! - record 1: w0 = [`MODEL`] | [`CONTINUED`] | tint rgb (7 bits each),
//!   w1 = c1.y | c1.z << 16, w2 = c3.x | c3.y << 16
//! - record 2: w0 = [`MODEL`] | [`CONTINUED`] | u0 | v0 << 10 | u1 << 20,
//!   w1 = c3.z | v1 << 16, w2 = u3 | v3 << 10
//!
//! Coordinates are section-local in 1/1024 block with one block of margin either side; texture
//! coordinates are in 1/32 texel. `kind` is an axis face, or [`UNSHADED`].

use super::quad::{Face, Quad, Surface};

/// Set in w0 of every record of a model face.
pub const MODEL: u32 = 1 << 31;
/// Set in w0 of the records after a model face's first; they draw nothing themselves.
pub const CONTINUED: u32 = 1 << 30;
/// Kind of a face without directional shade, lit by the cell it is in.
pub const UNSHADED: Face = 6;

fn coordinate(v: f32) -> u32 {
    ((v + 16.0) * 64.0).round().clamp(0.0, 65535.0) as u32
}

fn texel(v: f32) -> u32 {
    (v * 32.0).round().clamp(0.0, 1023.0) as u32
}

/// `corners` in section-local 1/16 block and `uv` in texels, at corners 0, 1 and 3; `ao` at all four.
pub fn records(corners: [[f32; 3]; 3], uv: [[f32; 2]; 3], kind: Face, surface: Surface, ao: [u8; 4]) -> [Quad; 3] {
    let [c0, c1, c3] = corners.map(|c| c.map(coordinate));
    let [[u0, v0], [u1, v1], [u3, v3]] = uv.map(|t| t.map(texel));
    let [r, g, b] = surface.color.map(u32::from);
    let flip = u32::from((ao[0] + ao[2]) < (ao[1] + ao[3]));
    let ao = ao.iter().enumerate().fold(0, |a, (i, &v)| a | (u32::from(v) << (i * 2)));
    let look = u32::from(kind) | ao << 4 | (surface.material as u32) << 12 | surface.tint_kind << 14 | u32::from(surface.texture) << 16 | flip << 28;
    [
        [MODEL | look, c0[0] | c0[1] << 16, c0[2] | c1[0] << 16],
        [MODEL | CONTINUED | r | g << 7 | b << 14, c1[1] | c1[2] << 16, c3[0] | c3[1] << 16],
        [MODEL | CONTINUED | u0 | v0 << 10 | u1 << 20, c3[2] | v1 << 16, u3 | v3 << 10],
    ]
    .map(Quad)
}

/// Corner 0 of a model face's first record along `axis`, in 1/16 block like a plain quad's position.
pub(super) fn plane(head: &Quad, axis: usize) -> u32 {
    let raw = [head.0[1] & 0xffff, head.0[1] >> 16, head.0[2] & 0xffff][axis];
    (raw / 64).saturating_sub(16).min(511)
}

/// Sorts translucent quads by [`Quad::blend_order`], keeping each model face's records together.
pub fn sort_for_blending(quads: &mut Vec<Quad>) {
    let mut faces: Vec<&[Quad]> = quads.chunk_by(|_, next| next.0[0] & (MODEL | CONTINUED) == MODEL | CONTINUED).collect();
    faces.sort_by_key(|records| records[0].blend_order());
    *quads = faces.concat();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::Material;

    const SURFACE: Surface = Surface { texture: 300, tint_kind: 1, material: Material::Cutout, color: [1, 2, 3], turn: 0 };

    #[test]
    fn records_hold_what_the_shader_reads() {
        let corners = [[0.0, 16.0, 0.0], [0.0, 16.0, 16.0], [272.0, -16.0, 8.5]];
        let [head, tint, uv] = records(corners, [[0.0, 0.0], [0.0, 16.0], [15.5, 0.25]], 2, SURFACE, [0, 3, 1, 2]).map(|q| q.0);
        assert_eq!(head[0], MODEL | 2 | 0b10_01_11_00 << 4 | 1 << 12 | 1 << 14 | 300 << 16 | 1 << 28);
        assert_eq!((head[1] & 0xffff, head[1] >> 16, head[2] & 0xffff), (1024, 2048, 1024));
        assert_eq!(tint[0], MODEL | CONTINUED | 1 | 2 << 7 | 3 << 14);
        assert_eq!((tint[2] & 0xffff, tint[2] >> 16, uv[1] & 0xffff), (18432, 0, 1568));
        assert_eq!((uv[0] >> 20 & 1023, uv[1] >> 16, uv[2] & 1023, uv[2] >> 10), (0, 512, 496, 8));
    }

    #[test]
    fn blending_order_moves_a_model_face_whole() {
        let face = |y: f32| records([[0.0, y, 0.0], [0.0, y, 16.0], [16.0, y, 0.0]], [[0.0; 2]; 3], 2, SURFACE, [3; 4]);
        let (near, far) = (face(32.0), face(16.0));
        let plain = Quad::new([0, 24, 0], 2, [16, 16], SURFACE, [3; 4]);
        let mut quads: Vec<Quad> = near.into_iter().chain([plain]).chain(far).collect();
        sort_for_blending(&mut quads);
        // Up faces blend from the lowest plane: far (16), the plain quad (24), near (32).
        assert_eq!(quads, far.into_iter().chain([plain]).chain(near).collect::<Vec<_>>());
    }
}
