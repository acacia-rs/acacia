//! `FlowingFluid.getFlow`: which way the surface texture runs.

use super::super::Ctx;
use super::tesselate::fluid_at;
use crate::blocks::{Fluid, RenderBlock};

/// `Direction.Plane.HORIZONTAL` (north, east, south, west) with the face index of each.
const HORIZONTAL: [([i32; 3], usize); 4] = [([0, 0, -1], 5), ([1, 0, 0], 0), ([0, 0, 1], 4), ([-1, 0, 0], 1)];

fn normalize([x, y, z]: [f64; 3]) -> [f64; 3] {
    let length = (x * x + y * y + z * z).sqrt();
    if length < 1.0e-5 { [0.0; 3] } else { [x / length, y / length, z / length] }
}

pub(super) fn at(ctx: &Ctx, [x, y, z]: [i32; 3], me: &RenderBlock) -> [f64; 3] {
    let fluid = me.fluid;
    let own = me.fluid_rules.own_height();
    let affects = |f: &RenderBlock| f.fluid == Fluid::None || f.fluid == fluid;
    let (mut fx, mut fz) = (0.0f64, 0.0f64);
    for ([dx, _, dz], _) in HORIZONTAL {
        let q = [x + dx, y, z + dz];
        let neighbour = fluid_at(ctx, q);
        if !affects(neighbour) {
            continue;
        }
        let mut height = neighbour.fluid_rules.own_height();
        let mut distance = 0.0f32;
        if height == 0.0 {
            if !ctx.block(q).fluid_rules.blocks_motion {
                let below = fluid_at(ctx, [q[0], y - 1, q[2]]);
                if affects(below) {
                    height = below.fluid_rules.own_height();
                    if height > 0.0 {
                        distance = own - (height - 0.8888889);
                    }
                }
            }
        } else if height > 0.0 {
            distance = own - height;
        }
        if distance != 0.0 {
            fx += f64::from(dx as f32 * distance);
            fz += f64::from(dz as f32 * distance);
        }
    }
    let mut flow = [fx, 0.0, fz];
    if me.fluid_rules.falling {
        // isSolidFace: the neighbour's far face, unless it holds the same fluid or is ice.
        let solid_face = |q: [i32; 3], face: usize| {
            let block = ctx.block(q);
            fluid_at(ctx, q).fluid != fluid && !block.fluid_rules.ice && block.fluid_rules.sturdy(face)
        };
        if HORIZONTAL.iter().any(|&([dx, _, dz], face)| solid_face([x + dx, y, z + dz], face) || solid_face([x + dx, y + 1, z + dz], face)) {
            let [nx, ny, nz] = normalize(flow);
            flow = [nx, ny - 6.0, nz];
        }
    }
    normalize(flow)
}
