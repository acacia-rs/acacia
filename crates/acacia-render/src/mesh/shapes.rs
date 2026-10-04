//! Non-cube blocks, one quad per visible face: collision boxes, crossed planes, liquids.

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
        let records = model::records(corners, f.uv, f.shade.unwrap_or(model::UNSHADED), surface);
        if b.layer == Layer::Translucent { out.translucent.extend(records) } else { out.solid.extend(records) }
    }
}

fn emit_cross(ctx: &Ctx, p: [i32; 3], b: &RenderBlock, out: &mut SectionMesh) {
    let pos = p.map(|c| (c * 16) as u32);
    let surface = Surface { material: Material::Cutout, ..ctx.surface(p, b, 0) };
    for face in 6..10 {
        push(out, b, Quad::new(pos, face, [16, 16], surface, NO_AO));
    }
}

