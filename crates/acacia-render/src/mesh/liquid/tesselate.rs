//! `FluidRenderer.tesselate`, rule for rule.

use super::super::Ctx;
use super::super::shapes::neighbour;
use super::{LiquidFace, Sprite, flow};
use crate::blocks::{Fluid, RenderBlock};
use crate::mth;

const MAX_FLUID_HEIGHT: f32 = 0.8888889;
const OFFS: f32 = 0.001;
const EAST: usize = 0;
const WEST: usize = 1;
const UP: usize = 2;
const DOWN: usize = 3;
const SOUTH: usize = 4;
const NORTH: usize = 5;

/// Water or lava in the block layer, or water in the liquid layer of a waterlogged block.
pub(in crate::mesh) fn fluid_at<'a>(ctx: &'a Ctx, q: [i32; 3]) -> &'a RenderBlock {
    let block = ctx.block(q);
    if block.fluid != Fluid::None { block } else { ctx.table.get(ctx.v.liquid(q[0], q[1], q[2])) }
}

/// `isFaceOccludedByState`: a fluid box `height` tall against `state`'s face opposite `face`.
fn occluded_by(face: usize, height: f32, state: &RenderBlock) -> bool {
    let rules = &state.fluid_rules;
    match face {
        UP => f64::from(height) >= 1.0 - 1e-7 && rules.occluding(DOWN) >= 1.0,
        DOWN => rules.occluding(UP) >= 1.0,
        _ => {
            let cover = rules.occluding(face ^ 1);
            cover > 0.0 && f64::from(height) <= f64::from(cover) + 1e-7
        }
    }
}

pub(in crate::mesh) fn faces(ctx: &Ctx, p: [i32; 3]) -> Vec<LiquidFace> {
    let me = fluid_at(ctx, p);
    let fluid = me.fluid;
    if fluid == Fluid::None {
        return Vec::new();
    }
    let own_block = ctx.block(p);
    let same = |q: [i32; 3]| fluid_at(ctx, q).fluid == fluid;
    let next = |face: usize| neighbour(p, face);
    let should_render = |face: usize| !same(next(face)) && own_block.fluid_rules.occluding(face) < 1.0;
    let render_up = !same(next(UP));
    let render_down = should_render(DOWN) && !occluded_by(DOWN, MAX_FLUID_HEIGHT, ctx.block(next(DOWN)));
    let sides = [NORTH, SOUTH, WEST, EAST].map(|face| (face, should_render(face)));
    if !render_up && !render_down && !sides.iter().any(|s| s.1) {
        return Vec::new();
    }
    // getHeight: 1 under more of the fluid, -1 where a solid block holds the surface up.
    let height = |[x, y, z]: [i32; 3]| -> f32 {
        let f = fluid_at(ctx, [x, y, z]);
        if f.fluid == fluid {
            if same([x, y + 1, z]) { 1.0 } else { f.fluid_rules.own_height() }
        } else if !ctx.block([x, y, z]).fluid_rules.solid {
            0.0
        } else {
            -1.0
        }
    };
    let [x, y, z] = p;
    let own = height(p);
    let (mut ne, mut nw, mut se, mut sw) = (1.0f32, 1.0f32, 1.0f32, 1.0f32);
    if own < 1.0 {
        let (n, s, e, w) = (height([x, y, z - 1]), height([x, y, z + 1]), height([x + 1, y, z]), height([x - 1, y, z]));
        // calculateAverageHeight(self, height2, height1, corner): nearly full cells weigh ten times more.
        let average = |h2: f32, h1: f32, corner: [i32; 3]| -> f32 {
            if h1 >= 1.0 || h2 >= 1.0 {
                return 1.0;
            }
            let mut sum = [0f32; 2];
            let mut add = |h: f32| {
                if h >= 0.8 {
                    sum[0] += h * 10.0;
                    sum[1] += 10.0;
                } else if h >= 0.0 {
                    sum[0] += h;
                    sum[1] += 1.0;
                }
            };
            if h1 > 0.0 || h2 > 0.0 {
                let hc = height(corner);
                if hc >= 1.0 {
                    return 1.0;
                }
                add(hc);
            }
            add(own);
            add(h1);
            add(h2);
            sum[0] / sum[1]
        };
        ne = average(n, e, [x + 1, y, z - 1]);
        nw = average(n, w, [x - 1, y, z - 1]);
        se = average(s, e, [x + 1, y, z + 1]);
        sw = average(s, w, [x - 1, y, z + 1]);
    }
    let tint = ctx.tint(p, me.tint[UP]);
    let mut out = Vec::new();
    let mut add = |corners: [[f32; 3]; 4], uv: [[f32; 2]; 4], sprite, face, light, back: bool| {
        out.push(LiquidFace { corners, uv, sprite, face, tint, light });
        if back {
            out.push(LiquidFace { corners: [0, 3, 2, 1].map(|i| corners[i]), uv: [0, 3, 2, 1].map(|i| uv[i]), sprite, face, tint, light });
        }
    };
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let bottom = if render_down { OFFS } else { 0.0 };
    if render_up && !occluded_by(UP, nw.min(sw).min(se.min(ne)), ctx.block(next(UP))) {
        nw -= OFFS;
        sw -= OFFS;
        se -= OFFS;
        ne -= OFFS;
        let flow = flow::at(ctx, p, me);
        let (sprite, uv) = if flow[0] == 0.0 && flow[2] == 0.0 {
            (Sprite::Still, [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]])
        } else {
            let angle = mth::atan2(flow[2], flow[0]) as f32 - std::f64::consts::FRAC_PI_2 as f32;
            let s = mth::sin(f64::from(angle)) * 0.25;
            let c = mth::cos(f64::from(angle)) * 0.25;
            let uv = [[-c - s, -c + s], [-c + s, c + s], [c + s, c - s], [c - s, -c - s]];
            (Sprite::Flow, uv.map(|t| t.map(|v| 0.5 + v)))
        };
        // FluidState.shouldRenderBackwardUpFace over the 3x3 cells above.
        let backward = (-1..=1).any(|ox| (-1..=1).any(|oz| !same([x + ox, y + 1, z + oz]) && !ctx.block([x + ox, y + 1, z + oz]).occludes));
        let corners = [[fx, fy + nw, fz], [fx, fy + sw, fz + 1.0], [fx + 1.0, fy + se, fz + 1.0], [fx + 1.0, fy + ne, fz]];
        add(corners, uv, sprite, UP, p, backward);
    }
    if render_down {
        let y0 = fy + bottom;
        let corners = [[fx, y0, fz], [fx + 1.0, y0, fz], [fx + 1.0, y0, fz + 1.0], [fx, y0, fz + 1.0]];
        add(corners, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], Sprite::Still, DOWN, [x, y - 1, z], false);
    }
    for (face, render) in sides {
        let (h0, h1, [x0, z0], [x1, z1]) = match face {
            NORTH => (nw, ne, [fx, fz + OFFS], [fx + 1.0, fz + OFFS]),
            SOUTH => (se, sw, [fx + 1.0, fz + 1.0 - OFFS], [fx, fz + 1.0 - OFFS]),
            WEST => (sw, nw, [fx + OFFS, fz + 1.0], [fx + OFFS, fz]),
            _ => (ne, se, [fx + 1.0 - OFFS, fz], [fx + 1.0 - OFFS, fz + 1.0]),
        };
        let against = ctx.block(next(face));
        if !render || occluded_by(face, h0.max(h1), against) {
            continue;
        }
        let overlay = fluid == Fluid::Water && against.fluid_rules.overlay;
        let (v0, v1) = ((1.0 - h0) * 0.5, (1.0 - h1) * 0.5);
        let corners = [[x0, fy + h0, z0], [x1, fy + h1, z1], [x1, fy + bottom, z1], [x0, fy + bottom, z0]];
        let sprite = if overlay { Sprite::Overlay } else { Sprite::Flow };
        add(corners, [[0.0, v0], [0.5, v1], [0.5, 0.5], [0.0, 0.5]], sprite, face, p, !overlay);
    }
    out
}
