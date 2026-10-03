//! Greedy meshing of full-cube faces: per face direction and slice, visible faces with identical
//! surface and ambient occlusion merge into rectangles.

use super::quad::{AXES, DIRS, Quad, Surface};
use super::volume::Volume;
use super::SectionMesh;
use crate::blocks::{BlockTable, Layer, Material, Shape, Tint};

/// Mask entry: 0 = no face, else `key | PRESENT`.
const PRESENT: u32 = 1 << 31;
const TRANSLUCENT: u32 = 1 << 30;

pub fn cubes(v: &Volume, table: &BlockTable, out: &mut SectionMesh) {
    let mut mask = [0u32; 256];
    for face in 0..6u8 {
        let (axis, ua, va) = AXES[face as usize];
        let dir = DIRS[face as usize];
        for s in 0..16 {
            for vv in 0..16 {
                for u in 0..16 {
                    let mut p = [0i32; 3];
                    (p[axis], p[ua], p[va]) = (s, u, vv);
                    mask[(vv * 16 + u) as usize] = face_key(v, table, p, dir, face);
                }
            }
            emit(&mut mask, face, s, out);
        }
    }
}

fn face_key(v: &Volume, table: &BlockTable, p: [i32; 3], dir: [i32; 3], face: u8) -> u32 {
    let id = v.block(p[0], p[1], p[2]);
    let b = table.get(id);
    if b.shape != Shape::Cube || b.layer == Layer::Invisible {
        return 0;
    }
    let n = [p[0] + dir[0], p[1] + dir[1], p[2] + dir[2]];
    let nid = v.block(n[0], n[1], n[2]);
    let nb = table.get(nid);
    if nb.occludes || (nid == id && b.cull_same) {
        return 0;
    }
    let f = face as usize;
    let ao = if b.layer == Layer::Solid { corner_ao(v, table, n, face) } else { [3; 4] };
    let ao_bits = ao.iter().enumerate().fold(0, |a, (i, &x)| a | (u32::from(x) << (i * 2)));
    let translucent = if b.layer == Layer::Translucent { TRANSLUCENT } else { 0 };
    PRESENT | translucent | u32::from(b.textures[f]) | (b.tint[f] as u32) << 12 | (b.material[f] as u32) << 14 | ao_bits << 16
}

/// AO per corner for a face whose front cell is `n`: each corner looks at its two edge
/// neighbours and the diagonal one in that plane.
pub fn corner_ao(v: &Volume, table: &BlockTable, n: [i32; 3], face: u8) -> [u8; 4] {
    let (_, ua, va) = AXES[face as usize];
    let occ = |du: i32, dv: i32| {
        let mut q = n;
        q[ua] += du;
        q[va] += dv;
        u8::from(table.get(v.block(q[0], q[1], q[2])).occludes)
    };
    [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|(du, dv)| {
        let (s1, s2, c) = (occ(du, 0), occ(0, dv), occ(du, dv));
        if s1 == 1 && s2 == 1 { 0 } else { 3 - s1 - s2 - c }
    })
}

fn emit(mask: &mut [u32; 256], face: u8, s: i32, out: &mut SectionMesh) {
    let (axis, ua, va) = AXES[face as usize];
    let plane = if face.is_multiple_of(2) { s + 1 } else { s };
    for vv in 0..16usize {
        let mut u = 0usize;
        while u < 16 {
            let key = mask[vv * 16 + u];
            if key == 0 {
                u += 1;
                continue;
            }
            let w = (u..16).take_while(|&x| mask[vv * 16 + x] == key).count();
            let h = (vv..16).take_while(|&y| (u..u + w).all(|x| mask[y * 16 + x] == key)).count();
            for y in vv..vv + h {
                mask[y * 16 + u..y * 16 + u + w].fill(0);
            }
            let mut pos = [0u32; 3];
            (pos[axis], pos[ua], pos[va]) = (plane as u32 * 16, u as u32 * 16, vv as u32 * 16);
            let surface = Surface {
                texture: (key & 0xfff) as u16,
                tint: tint_from((key >> 12) & 3),
                material: material_from((key >> 14) & 3),
            };
            let ao = [0, 1, 2, 3].map(|i| ((key >> (16 + i * 2)) & 3) as u8);
            let quad = Quad::new(pos, face, [w as u32 * 16, h as u32 * 16], surface, ao);
            if key & TRANSLUCENT != 0 { out.translucent.push(quad) } else { out.solid.push(quad) }
            u += w;
        }
    }
}

fn tint_from(v: u32) -> Tint {
    [Tint::None, Tint::Grass, Tint::Foliage, Tint::Water][v as usize]
}

fn material_from(v: u32) -> Material {
    [Material::Opaque, Material::Cutout, Material::Blend, Material::Overlay][v as usize]
}
