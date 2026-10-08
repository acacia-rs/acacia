//! Flat item sprites as 3D meshes, after Java's `ItemModelGenerator`: a front and a back quad over
//! the whole sprite, and a side face wherever an opaque texel borders a transparent one.

use glam::{Mat4, Vec3};

use crate::entity::bake::{Joint, Mesh, Vertex};

/// Java's item slab: z 7.5 to 8.5 of 16.
const HALF_DEPTH: f32 = 0.5 / 16.0;
/// Side faces sample this far inside their texel, so filtering never reaches a neighbour.
const UV_SHRINK: f32 = 0.1;

#[derive(Clone, Copy)]
enum Side {
    Up,
    Down,
    Left,
    Right,
}

/// The sprite's mesh in model space (blocks, centred on the origin, facing -z), one bone. `rgba`
/// is one frame, `width` × `height`; alpha 0 is transparent, as in Java.
pub fn extrude(width: u32, height: u32, rgba: &[u8]) -> Mesh {
    let (w, h) = (width as i32, height as i32);
    let transparent = |x: i32, y: i32| x < 0 || y < 0 || x >= w || y >= h || rgba[(y * w + x) as usize * 4 + 3] == 0;
    let mut vertices = Vec::new();
    quad(&mut vertices, [0.0, 0.0], [1.0, 1.0], Vec3::Z, [[0.0, 0.0], [1.0, 1.0]]);
    quad(&mut vertices, [0.0, 0.0], [1.0, 1.0], Vec3::NEG_Z, [[0.0, 0.0], [1.0, 1.0]]);
    for y in 0..h {
        for x in 0..w {
            if transparent(x, y) {
                continue;
            }
            let neighbours = [(Side::Up, x, y - 1), (Side::Down, x, y + 1), (Side::Left, x - 1, y), (Side::Right, x + 1, y)];
            for (side, nx, ny) in neighbours {
                if transparent(nx, ny) {
                    side_face(&mut vertices, side, x, y, width, height);
                }
            }
        }
    }
    let joint = Joint { parent: None, pivot: Vec3::ZERO, rotation: [0.0; 3], unbind: Mat4::IDENTITY };
    Mesh { vertices, bones: vec!["root".into()], joints: vec![joint] }
}

fn side_face(out: &mut Vec<Vertex>, side: Side, x: i32, y: i32, width: u32, height: u32) {
    let (tx, ty) = (1.0 / width as f32, 1.0 / height as f32);
    let (u0, v0) = (x as f32 * tx, y as f32 * ty);
    let uv = [[u0 + UV_SHRINK * tx, v0 + UV_SHRINK * ty], [u0 + (1.0 - UV_SHRINK) * tx, v0 + (1.0 - UV_SHRINK) * ty]];
    // Sprite space: x right, y down, both 0..1.
    let (left, right, top, bottom) = (u0, u0 + tx, v0, v0 + ty);
    let (from, to, normal) = match side {
        Side::Up => ([left, top], [right, top], Vec3::Y),
        Side::Down => ([left, bottom], [right, bottom], Vec3::NEG_Y),
        Side::Left => ([left, top], [left, bottom], Vec3::NEG_X),
        Side::Right => ([right, top], [right, bottom], Vec3::X),
    };
    quad(out, from, to, normal, uv);
}

/// A face between two sprite-space corners: across the slab for the front and back (normal ±z),
/// an edge extruded through the slab's depth otherwise.
fn quad(out: &mut Vec<Vertex>, from: [f32; 2], to: [f32; 2], normal: Vec3, uv: [[f32; 2]; 2]) {
    // Sprite space to model space: x centred, y up, and z mirrored (models face -z).
    let at = |[sx, sy]: [f32; 2], z: f32| [sx - 0.5, 0.5 - sy, -z];
    let corners = if normal.z != 0.0 {
        let z = normal.z * HALF_DEPTH;
        [at(from, z), at([to[0], from[1]], z), at(to, z), at([from[0], to[1]], z)]
    } else {
        [at(from, HALF_DEPTH), at(to, HALF_DEPTH), at(to, -HALF_DEPTH), at(from, -HALF_DEPTH)]
    };
    let [[u0, v0], [u1, v1]] = uv;
    let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    let normal = Vec3::new(normal.x, normal.y, -normal.z).to_array();
    out.extend([0, 1, 2, 0, 2, 3].map(|i| Vertex { position: corners[i], bone: 0, normal, uv: uvs[i] }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(size: u32, opaque: &[(u32, u32)]) -> Vec<u8> {
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        for &(x, y) in opaque {
            rgba[((y * size + x) * 4 + 3) as usize] = 255;
        }
        rgba
    }

    #[test]
    fn one_texel_gets_four_sides() {
        let mesh = extrude(16, 16, &sprite(16, &[(3, 4)]));
        assert_eq!(mesh.vertices.len(), 6 * (2 + 4));
        let bounds = |axis: usize| mesh.vertices.iter().map(|v| v.position[axis]).fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p), b.max(p)));
        assert_eq!(bounds(0), (-0.5, 0.5));
        assert_eq!(bounds(2), (-HALF_DEPTH, HALF_DEPTH));
    }

    #[test]
    fn touching_texels_share_no_side() {
        let mesh = extrude(16, 16, &sprite(16, &[(3, 4), (4, 4)]));
        assert_eq!(mesh.vertices.len(), 6 * (2 + 6));
    }

    #[test]
    fn side_faces_sit_on_the_texel_edge() {
        let mesh = extrude(16, 16, &sprite(16, &[(0, 0)]));
        let up: Vec<_> = mesh.vertices.iter().filter(|v| v.normal == [0.0, 1.0, 0.0]).collect();
        assert!(up.iter().all(|v| v.position[1] == 0.5 && (-0.5..=-0.5 + 1.0 / 16.0).contains(&v.position[0])));
    }
}
