//! Non-cube blocks, one quad per visible face: collision boxes, crossed planes, liquids.

use super::greedy::corner_ao;
use super::liquid::liquid;
use super::quad::{AXES, DIRS, Quad, Surface, quantize};
use super::{Ctx, SectionMesh, model};
use crate::blocks::{Box16, Layer, Material, ModelFace, RenderBlock, Shape};

pub(super) const NO_AO: [u8; 4] = [3; 4];

pub(super) fn others(ctx: &Ctx, out: &mut SectionMesh) {
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..16 {
                let p = [x, y, z];
                let b = ctx.block(p);
                match &b.shape {
                    Shape::Boxes(boxes) => boxes.iter().for_each(|bx| emit_box(ctx, p, bx, b, out)),
                    Shape::Cross => emit_cross(ctx, p, b, out),
                    Shape::Model(faces) => emit_model(ctx, p, faces, b, out),
                    _ => {}
                }
                out.models.extend(b.model.iter().map(|m| (p.map(|c| c as u8), m.clone())));
                liquid(ctx, p, out);
            }
        }
    }
}

pub(super) fn push(out: &mut SectionMesh, b: &RenderBlock, quad: Quad) {
    if b.layer == Layer::Translucent { out.translucent.push(quad) } else { out.solid.push(quad) }
}

pub(super) fn neighbour(p: [i32; 3], face: usize) -> [i32; 3] {
    let d = DIRS[face];
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

fn emit_box(ctx: &Ctx, p: [i32; 3], bx: &Box16, b: &RenderBlock, out: &mut SectionMesh) {
    let (min, max) = ([bx[0], bx[1], bx[2]], [bx[3], bx[4], bx[5]]);
    for (face, &(axis, ua, va)) in AXES.iter().enumerate() {
        let positive = face.is_multiple_of(2);
        let size = [max[ua] - min[ua], max[va] - min[va]];
        if size.contains(&0) {
            continue;
        }
        let on_edge = if positive { max[axis] == 16 } else { min[axis] == 0 };
        if on_edge && ctx.block(neighbour(p, face)).occludes {
            continue;
        }
        let mut pos = [0u32; 3];
        pos[axis] = (p[axis] * 16) as u32 + u32::from(if positive { max[axis] } else { min[axis] });
        pos[ua] = (p[ua] * 16) as u32 + u32::from(min[ua]);
        pos[va] = (p[va] * 16) as u32 + u32::from(min[va]);
        let quad = Quad::new(pos, face as u8, [u32::from(size[0]), u32::from(size[1])], ctx.surface(p, b, face), NO_AO);
        push(out, b, quad);
    }
}

fn emit_model(ctx: &Ctx, p: [i32; 3], faces: &[ModelFace], b: &RenderBlock, out: &mut SectionMesh) {
    let origin = p.map(|c| (c * 16) as f32);
    for f in faces {
        if f.cull.is_some_and(|side| ctx.block(neighbour(p, usize::from(side))).occludes) {
            continue;
        }
        let corners = f.corners.map(|c| [c[0] + origin[0], c[1] + origin[1], c[2] + origin[2]]);
        let surface = Surface { texture: f.texture, tint_kind: f.tint.shader_kind(), material: f.material, color: quantize(ctx.tint(p, f.tint)) };
        let ao = match f.shade {
            Some(side) if f.ambient_occlusion && b.layer == Layer::Solid => model_ao(ctx, p, f, side),
            _ => NO_AO,
        };
        let records = model::records(corners, f.uv, f.shade.unwrap_or(model::UNSHADED), surface, ao);
        if b.layer == Layer::Translucent { out.translucent.extend(records) } else { out.solid.extend(records) }
    }
}

/// AO at a model face's corners, as Java does it: the corner values of a cell's `side` (the cell in
/// front for a face on the block's side, else the block's own), interpolated to where each corner is.
fn model_ao(ctx: &Ctx, p: [i32; 3], f: &ModelFace, side: u8) -> [u8; 4] {
    let (axis, ua, va) = AXES[usize::from(side)];
    let [c0, c1, c3] = f.corners;
    let c2 = [0, 1, 2].map(|i| c1[i] + c3[i] - c0[i]);
    let edge = if side.is_multiple_of(2) { 16.0 } else { 0.0 };
    let cell = if (c0[axis] - edge).abs() < 0.01 { neighbour(p, usize::from(side)) } else { p };
    let at = corner_ao(ctx, cell, side).map(f32::from);
    [c0, c1, c2, c3].map(|c| {
        let (u, v) = ((c[ua] / 16.0).clamp(0.0, 1.0), (c[va] / 16.0).clamp(0.0, 1.0));
        let (low, high) = (at[0] + (at[1] - at[0]) * u, at[3] + (at[2] - at[3]) * u);
        (low + (high - low) * v).round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::super::volume::{SIDE, Volume, cell};
    use super::*;
    use crate::biome::BiomeColors;
    use crate::blocks::{BlockTable, Tint};

    /// Ids: 0 air, 1 stone, 2 a slab's top face (the west half of it when `half`).
    fn slab_top_ao(half: bool, stone: &[[i32; 3]]) -> u32 {
        let x1 = if half { 8.0 } else { 16.0 };
        let face = ModelFace {
            corners: [[0.0, 8.0, 0.0], [x1, 8.0, 0.0], [0.0, 8.0, 16.0]],
            uv: [[0.0; 2]; 3],
            texture: 0,
            tint: Tint::None,
            material: Material::Opaque,
            shade: Some(2),
            ambient_occlusion: true,
            cull: None,
        };
        let air = RenderBlock { shape: Shape::None, occludes: false, ..BlockTable::cube(0) };
        let slab = RenderBlock { shape: Shape::Model([face].into()), occludes: false, ..BlockTable::cube(0) };
        let table = BlockTable::from_blocks(vec![air, BlockTable::cube(0), slab]);
        let cells = || vec![0u32; SIDE * SIDE * SIDE].into_boxed_slice().try_into().unwrap();
        let mut v = Volume { blocks: cells(), liquid: cells(), biomes: cells() };
        v.blocks[cell(5, 5, 5)] = 2;
        stone.iter().for_each(|&[x, y, z]| v.blocks[cell(x, y, z)] = 1);
        let mut out = SectionMesh::default();
        others(&Ctx { v: &v, table: &table, biomes: &BiomeColors::default() }, &mut out);
        out.solid[0].0[0] >> 4 & 0xff
    }

    #[test]
    fn a_model_face_inside_its_block_is_occluded_by_the_blocks_beside_it() {
        assert_eq!(slab_top_ao(false, &[]), 0xff);
        // Corners in face order: (x0,z0), (x1,z0), (x1,z1), (x0,z1); stone to the east darkens x1.
        assert_eq!(slab_top_ao(false, &[[6, 5, 5]]), 0b11_10_10_11);
        assert_eq!(slab_top_ao(false, &[[6, 6, 5]]), 0xff);
        // Halfway to the darkened corners: 2.5 rounds up to unoccluded.
        assert_eq!(slab_top_ao(true, &[[6, 5, 5]]), 0xff);
        assert_eq!(slab_top_ao(true, &[[6, 5, 5], [5, 5, 4]]), 0b11_11_01_10);
    }
}

fn emit_cross(ctx: &Ctx, p: [i32; 3], b: &RenderBlock, out: &mut SectionMesh) {
    let pos = p.map(|c| (c * 16) as u32);
    let surface = Surface { material: Material::Cutout, ..ctx.surface(p, b, 0) };
    for face in 6..10 {
        push(out, b, Quad::new(pos, face, [16, 16], surface, NO_AO));
    }
}

