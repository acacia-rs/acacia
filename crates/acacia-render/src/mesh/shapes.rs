//! Non-cube blocks, one quad per visible face: collision boxes, crossed planes, liquids.

use super::SectionMesh;
use super::quad::{AXES, DIRS, Quad, Surface};
use super::volume::Volume;
use crate::blocks::{Box16, BlockTable, Fluid, Layer, Material, RenderBlock, Shape};

const NO_AO: [u8; 4] = [3; 4];

pub fn others(v: &Volume, table: &BlockTable, out: &mut SectionMesh) {
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..16 {
                let p = [x, y, z];
                let b = table.get(v.block(x, y, z));
                match &b.shape {
                    Shape::Boxes(boxes) => boxes.iter().for_each(|bx| emit_box(v, table, p, bx, b, out)),
                    Shape::Cross => emit_cross(p, b, out),
                    _ => {}
                }
                liquid(v, table, p, out);
            }
        }
    }
}

fn surface(b: &RenderBlock, face: usize) -> Surface {
    Surface { texture: b.textures[face], tint: b.tint[face], material: b.material[face] }
}

fn push(out: &mut SectionMesh, b: &RenderBlock, quad: Quad) {
    if b.layer == Layer::Translucent { out.translucent.push(quad) } else { out.solid.push(quad) }
}

fn neighbour_occludes(v: &Volume, table: &BlockTable, p: [i32; 3], face: usize) -> bool {
    let d = DIRS[face];
    table.get(v.block(p[0] + d[0], p[1] + d[1], p[2] + d[2])).occludes
}

fn emit_box(v: &Volume, table: &BlockTable, p: [i32; 3], bx: &Box16, b: &RenderBlock, out: &mut SectionMesh) {
    let (min, max) = ([bx[0], bx[1], bx[2]], [bx[3], bx[4], bx[5]]);
    for (face, &(axis, ua, va)) in AXES.iter().enumerate() {
        let positive = face.is_multiple_of(2);
        let size = [max[ua] - min[ua], max[va] - min[va]];
        if size.contains(&0) {
            continue;
        }
        let on_edge = if positive { max[axis] == 16 } else { min[axis] == 0 };
        if on_edge && neighbour_occludes(v, table, p, face) {
            continue;
        }
        let mut pos = [0u32; 3];
        pos[axis] = (p[axis] * 16) as u32 + u32::from(if positive { max[axis] } else { min[axis] });
        pos[ua] = (p[ua] * 16) as u32 + u32::from(min[ua]);
        pos[va] = (p[va] * 16) as u32 + u32::from(min[va]);
        let quad = Quad::new(pos, face as u8, [u32::from(size[0]), u32::from(size[1])], surface(b, face), NO_AO);
        push(out, b, quad);
    }
}

fn emit_cross(p: [i32; 3], b: &RenderBlock, out: &mut SectionMesh) {
    let pos = p.map(|c| (c * 16) as u32);
    let surface = Surface { material: Material::Cutout, ..surface(b, 0) };
    for face in 6..10 {
        push(out, b, Quad::new(pos, face, [16, 16], surface, NO_AO));
    }
}

/// Water or lava in the block layer, or water in the liquid layer of a waterlogged block.
fn liquid(v: &Volume, table: &BlockTable, p: [i32; 3], out: &mut SectionMesh) {
    let [x, y, z] = p;
    let fluid_block = |b: &'_ RenderBlock| b.fluid != Fluid::None;
    let b = Some(table.get(v.block(x, y, z))).filter(|b| fluid_block(b));
    let Some(b) = b.or_else(|| Some(table.get(v.liquid(x, y, z))).filter(|b| fluid_block(b))) else { return };
    let fluid_at = |q: [i32; 3]| {
        let block = table.get(v.block(q[0], q[1], q[2]));
        if block.fluid != Fluid::None { block.fluid } else { table.get(v.liquid(q[0], q[1], q[2])).fluid }
    };
    let height = if fluid_at([x, y + 1, z]) == b.fluid { 16 } else { b.fluid_height.max(1) };
    for face in 0..6 {
        let d = DIRS[face];
        let n = [x + d[0], y + d[1], z + d[2]];
        if fluid_at(n) == b.fluid {
            continue;
        }
        let surface_below_top = face == 2 && height < 16;
        if !surface_below_top && table.get(v.block(n[0], n[1], n[2])).occludes {
            continue;
        }
        let (axis, ua, va) = AXES[face];
        let max = [16, height, 16];
        let mut pos = [0u32; 3];
        pos[axis] = (p[axis] * 16) as u32 + if face % 2 == 0 { u32::from(max[axis]) } else { 0 };
        pos[ua] = (p[ua] * 16) as u32;
        pos[va] = (p[va] * 16) as u32;
        let quad = Quad::new(pos, face as u8, [u32::from(max[ua]), u32::from(max[va])], surface(b, face), NO_AO);
        push(out, b, quad);
    }
}
