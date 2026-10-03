//! Non-cube blocks, one quad per visible face: collision boxes, crossed planes, liquids.

use super::quad::{AXES, DIRS, Quad, Surface};
use super::{Ctx, SectionMesh};
use crate::blocks::{Box16, Fluid, Layer, Material, RenderBlock, Shape};

const NO_AO: [u8; 4] = [3; 4];

pub(super) fn others(ctx: &Ctx, out: &mut SectionMesh) {
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..16 {
                let p = [x, y, z];
                let b = ctx.block(p);
                match &b.shape {
                    Shape::Boxes(boxes) => boxes.iter().for_each(|bx| emit_box(ctx, p, bx, b, out)),
                    Shape::Cross => emit_cross(ctx, p, b, out),
                    _ => {}
                }
                liquid(ctx, p, out);
            }
        }
    }
}

fn push(out: &mut SectionMesh, b: &RenderBlock, quad: Quad) {
    if b.layer == Layer::Translucent { out.translucent.push(quad) } else { out.solid.push(quad) }
}

fn neighbour(p: [i32; 3], face: usize) -> [i32; 3] {
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

fn emit_cross(ctx: &Ctx, p: [i32; 3], b: &RenderBlock, out: &mut SectionMesh) {
    let pos = p.map(|c| (c * 16) as u32);
    let surface = Surface { material: Material::Cutout, ..ctx.surface(p, b, 0) };
    for face in 6..10 {
        push(out, b, Quad::new(pos, face, [16, 16], surface, NO_AO));
    }
}

/// Water or lava in the block layer, or water in the liquid layer of a waterlogged block.
fn liquid(ctx: &Ctx, p: [i32; 3], out: &mut SectionMesh) {
    let [x, y, z] = p;
    let fluid = |q: [i32; 3]| {
        let block = ctx.block(q);
        if block.fluid != Fluid::None { block } else { ctx.table.get(ctx.v.liquid(q[0], q[1], q[2])) }
    };
    let b = fluid(p);
    if b.fluid == Fluid::None {
        return;
    }
    let height = if fluid([x, y + 1, z]).fluid == b.fluid { 16 } else { b.fluid_height.max(1) };
    for (face, &(axis, ua, va)) in AXES.iter().enumerate() {
        let n = neighbour(p, face);
        if fluid(n).fluid == b.fluid {
            continue;
        }
        let surface_below_top = face == 2 && height < 16;
        if !surface_below_top && ctx.block(n).occludes {
            continue;
        }
        let max = [16, height, 16];
        let mut pos = [0u32; 3];
        pos[axis] = (p[axis] * 16) as u32 + if face.is_multiple_of(2) { u32::from(max[axis]) } else { 0 };
        pos[ua] = (p[ua] * 16) as u32;
        pos[va] = (p[va] * 16) as u32;
        let quad = Quad::new(pos, face as u8, [u32::from(max[ua]), u32::from(max[va])], ctx.surface(p, b, face), NO_AO);
        push(out, b, quad);
    }
}
