//! Water and lava: the surface slopes between corner heights and its texture turns with the flow
//! (Java's `LiquidBlockRenderer` and `FlowingFluid::getFlow`).

use std::f32::consts::{FRAC_PI_2, TAU};

use super::quad::{AXES, DIRS, FULL_HEIGHT, LIQUID, Quad};
use super::shapes::{NO_AO, neighbour, push};
use super::{Ctx, SectionMesh};
use crate::blocks::{Fluid, Layer, RenderBlock, Shape};

/// Block corners in the order [`Quad::liquid`] stores their heights.
const CORNERS: [(i32, i32); 4] = [(0, 0), (1, 0), (1, 1), (0, 1)];
/// A source's `fluid_height`: what a liquid spilling over an edge is measured against.
const SOURCE_HEIGHT: i32 = 14;

/// Water or lava in the block layer, or water in the liquid layer of a waterlogged block.
fn fluid_at<'a>(ctx: &'a Ctx, q: [i32; 3]) -> &'a RenderBlock {
    let block = ctx.block(q);
    if block.fluid != Fluid::None { block } else { ctx.table.get(ctx.v.liquid(q[0], q[1], q[2])) }
}

/// Blocks a liquid spreads into; anything else holds a neighbouring surface up.
fn open(b: &RenderBlock) -> bool {
    matches!(b.shape, Shape::None | Shape::Cross)
}

pub(super) fn liquid(ctx: &Ctx, p: [i32; 3], out: &mut SectionMesh) {
    let b = fluid_at(ctx, p);
    if b.fluid == Fluid::None {
        return;
    }
    let heights = CORNERS.map(|(cx, cz)| corner_height(ctx, p, b.fluid, cx, cz));
    let full = heights == [FULL_HEIGHT; 4];
    let floor = p.map(|c| (c * 16) as u32);
    for (face, &(axis, ..)) in AXES.iter().enumerate() {
        let n = neighbour(p, face);
        if fluid_at(ctx, n).fluid == b.fluid {
            continue;
        }
        let against = ctx.block(n);
        // Ice and stained glass hide the liquid's sides too, or they show through as dark panes.
        let see_through_cube = face != 2 && against.shape == Shape::Cube && against.layer == Layer::Translucent;
        if (face != 2 || full) && (against.occludes || see_through_cube) {
            continue;
        }
        let mut pos = floor;
        match face {
            2 => {
                let turn = flow_turn(ctx, p, b);
                // Moving liquid shows its flowing (side) texture on top.
                let surface = |f| ctx.surface(p, b, if turn == 0 { f } else { 4 });
                push(out, b, Quad::liquid(pos, LIQUID + 2, heights, turn, surface(2)));
                if b.layer == Layer::Translucent {
                    // Back faces are culled, so the surface needs its own underside to show from below.
                    push(out, b, Quad::liquid(pos, LIQUID + 3, heights, turn, surface(3)));
                }
            }
            3 => push(out, b, Quad::new(pos, 3, [16, 16], ctx.surface(p, b, 3), NO_AO)),
            _ => {
                pos[axis] += if face.is_multiple_of(2) { 16 } else { 0 };
                push(out, b, Quad::liquid(pos, LIQUID + face as u8, heights, 0, ctx.surface(p, b, face)));
            }
        }
    }
}

/// Surface height of `fluid` in a cell as a fraction: 1 under more of it, 0 where it could spread,
/// `None` where a block holds the surface up.
fn height(ctx: &Ctx, [x, y, z]: [i32; 3], fluid: Fluid) -> Option<f32> {
    let b = fluid_at(ctx, [x, y, z]);
    if b.fluid == fluid {
        let covered = fluid_at(ctx, [x, y + 1, z]).fluid == fluid;
        return Some(if covered { 1.0 } else { f32::from(b.fluid_height) / 16.0 });
    }
    open(ctx.block([x, y, z])).then_some(0.0)
}

/// Height in 1/15 block of the corner at (`cx`, `cz`) of `p`, from the four cells that share it, so
/// neighbours agree. Nearly full cells weigh ten times more, which keeps lake edges from sagging.
fn corner_height(ctx: &Ctx, [x, y, z]: [i32; 3], fluid: Fluid, cx: i32, cz: i32) -> u8 {
    let (mut sum, mut weight) = (0.0, 0.0);
    for (dx, dz) in [(cx - 1, cz - 1), (cx, cz - 1), (cx - 1, cz), (cx, cz)] {
        match height(ctx, [x + dx, y, z + dz], fluid) {
            Some(h) if h >= 1.0 => return FULL_HEIGHT,
            Some(h) => {
                let w = if h >= 0.8 { 10.0 } else { 1.0 };
                sum += h * w;
                weight += w;
            }
            None => {}
        }
    }
    ((sum / weight * f32::from(FULL_HEIGHT)).round() as u8).max(1)
}

/// How far the surface texture turns to point downstream, as [`Quad::liquid`] stores it; 0 for
/// liquid at rest.
fn flow_turn(ctx: &Ctx, [x, y, z]: [i32; 3], b: &RenderBlock) -> u8 {
    let own = i32::from(b.fluid_height);
    let mut flow = [0i32; 2];
    for d in [DIRS[0], DIRS[1], DIRS[4], DIRS[5]] {
        let q = [x + d[0], y, z + d[2]];
        let n = fluid_at(ctx, q);
        let drop = if n.fluid == b.fluid {
            own - i32::from(n.fluid_height)
        } else if n.fluid == Fluid::None && open(ctx.block(q)) {
            // Over an edge: measure against the liquid one block down.
            let below = fluid_at(ctx, [q[0], y - 1, q[2]]);
            if below.fluid == b.fluid { own - (i32::from(below.fluid_height) - SOURCE_HEIGHT) } else { 0 }
        } else {
            0
        };
        flow = [flow[0] + d[0] * drop, flow[1] + d[2] * drop];
    }
    if flow == [0, 0] {
        return 0;
    }
    let angle = (flow[1] as f32).atan2(flow[0] as f32) - FRAC_PI_2;
    1 + ((angle.rem_euclid(TAU) / TAU * 255.0).round() as u32 % 255) as u8
}
