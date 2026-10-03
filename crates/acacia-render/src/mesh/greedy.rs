//! Greedy meshing of full-cube faces: per face direction and slice, visible faces with identical
//! surface and ambient occlusion merge into rectangles.

use super::quad::{AXES, DIRS, Quad, Surface};
use super::{Ctx, SectionMesh};
use crate::blocks::{Layer, Material, Shape};

/// Mask entry: 0 = no face. Bits: texture 0..12, tint kind 12..14, material 14..16, AO 16..24,
/// colour 24..45, translucent 45, present 46.
const PRESENT: u64 = 1 << 46;
const TRANSLUCENT: u64 = 1 << 45;

pub(super) fn cubes(ctx: &Ctx, out: &mut SectionMesh) {
    let mut mask = [0u64; 256];
    for face in 0..6u8 {
        let (axis, ua, va) = AXES[face as usize];
        for s in 0..16 {
            for vv in 0..16 {
                for u in 0..16 {
                    let mut p = [0i32; 3];
                    (p[axis], p[ua], p[va]) = (s, u, vv);
                    mask[(vv * 16 + u) as usize] = face_key(ctx, p, face);
                }
            }
            emit(&mut mask, face, s, out);
        }
    }
}

fn face_key(ctx: &Ctx, p: [i32; 3], face: u8) -> u64 {
    let id = ctx.v.block(p[0], p[1], p[2]);
    let b = ctx.table.get(id);
    if b.shape != Shape::Cube || b.layer == Layer::Invisible {
        return 0;
    }
    let dir = DIRS[face as usize];
    let n = [p[0] + dir[0], p[1] + dir[1], p[2] + dir[2]];
    let nid = ctx.v.block(n[0], n[1], n[2]);
    if ctx.table.get(nid).occludes || (nid == id && b.cull_same) {
        return 0;
    }
    let ao = if b.layer == Layer::Solid { corner_ao(ctx, n, face) } else { [3; 4] };
    let translucent = if b.layer == Layer::Translucent { TRANSLUCENT } else { 0 };
    PRESENT | translucent | pack(ctx.surface(p, b, face as usize), ao)
}

fn pack(s: Surface, ao: [u8; 4]) -> u64 {
    let ao_bits = ao.iter().enumerate().fold(0u64, |a, (i, &x)| a | (u64::from(x) << (i * 2)));
    let [r, g, b] = s.color.map(u64::from);
    u64::from(s.texture) | u64::from(s.tint_kind) << 12 | (s.material as u64) << 14 | ao_bits << 16 | (r | g << 7 | b << 14) << 24
}

fn unpack(key: u64) -> (Surface, [u8; 4]) {
    let bits = |at: u32, len: u32| (key >> at) & ((1 << len) - 1);
    let material = [Material::Opaque, Material::Cutout, Material::Blend, Material::Overlay][bits(14, 2) as usize];
    let surface = Surface {
        texture: bits(0, 12) as u16,
        tint_kind: bits(12, 2) as u32,
        material,
        color: [bits(24, 7) as u8, bits(31, 7) as u8, bits(38, 7) as u8],
    };
    (surface, [0, 1, 2, 3].map(|i| bits(16 + i * 2, 2) as u8))
}

/// AO per corner for a face whose front cell is `n`: each corner looks at its two edge
/// neighbours and the diagonal one in that plane.
pub(super) fn corner_ao(ctx: &Ctx, n: [i32; 3], face: u8) -> [u8; 4] {
    let (_, ua, va) = AXES[face as usize];
    let occ = |du: i32, dv: i32| {
        let mut q = n;
        q[ua] += du;
        q[va] += dv;
        u8::from(ctx.block(q).occludes)
    };
    [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|(du, dv)| {
        let (s1, s2, c) = (occ(du, 0), occ(0, dv), occ(du, dv));
        if s1 == 1 && s2 == 1 { 0 } else { 3 - s1 - s2 - c }
    })
}

fn emit(mask: &mut [u64; 256], face: u8, s: i32, out: &mut SectionMesh) {
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
            let (surface, ao) = unpack(key);
            let quad = Quad::new(pos, face, [w as u32 * 16, h as u32 * 16], surface, ao);
            if key & TRANSLUCENT != 0 { out.translucent.push(quad) } else { out.solid.push(quad) }
            u += w;
        }
    }
}
