//! Java's light and shade per vertex ([`crate::look::Lighting::Java`]): `BlockModelLighter`'s
//! smooth light and ambient occlusion with its partial-face weights, `LightCoordsUtil`, and
//! `ModelBlockRenderer`'s integer colour arithmetic. Ported from Horizon's `mesher/lighting.rs`
//! (MIT, see THIRD-PARTY.md) to the renderer's types.
//!
//! Directions here are Java's (`DOWN`..`EAST`); light coords are Java's packed ints, block level
//! << 4 | sky level << 20, each in 1/16 level after blending.

use super::Ctx;
use super::adjacency::ADJACENCY;
pub use crate::blocks::LightFacts;
use crate::light::data::LightCells;

/// Java's `Direction`, by ordinal.
pub type Dir = u8;
pub const DOWN: Dir = 0;
pub const UP: Dir = 1;
pub const NORTH: Dir = 2;
pub const SOUTH: Dir = 3;
pub const WEST: Dir = 4;
pub const EAST: Dir = 5;

const FULL_BRIGHT: u32 = 0x00F0_00F0;
const NORMALS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
/// Java's direction of each of the renderer's axis faces ([`super::quad::DIRS`] order).
pub(super) const OF_FACE: [Dir; 6] = [EAST, WEST, UP, DOWN, SOUTH, NORTH];

/// `CardinalLighting.DEFAULT` and `NETHER`.
pub(super) fn cardinal(nether: bool, d: Dir) -> f32 {
    match d {
        DOWN | UP if nether => 0.9,
        DOWN => 0.5,
        UP => 1.0,
        NORTH | SOUTH => 0.8,
        _ => 0.6,
    }
}

pub(super) fn step(p: [i32; 3], d: Dir) -> [i32; 3] {
    let n = NORMALS[usize::from(d)];
    [p[0] + n[0], p[1] + n[1], p[2] + n[2]]
}

/// `Direction.getApproximateNearest` of a quad's normal, up when it has none.
pub fn facing(positions: &[[f32; 3]; 4]) -> Dir {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let (a, b) = (sub(positions[1], positions[0]), sub(positions[2], positions[0]));
    let n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let mut best = (UP, 0.0f32);
    for (d, normal) in NORMALS.iter().enumerate() {
        let along = n[0] * normal[0] as f32 + n[1] * normal[1] as f32 + n[2] * normal[2] as f32;
        if along > best.1 {
            best = (d as Dir, along);
        }
    }
    best.0
}

/// A quad of the block at section-local `pos` to light: `positions` in block units, in Java's
/// vertex order.
pub struct QuadIn {
    pub pos: [i32; 3],
    pub positions: [[f32; 3]; 4],
    /// The direction it is shaded as.
    pub shade: Dir,
    pub cull: Option<Dir>,
    pub ambient_occlusion: bool,
}

/// What Java's vertices carry: light coords and grey per vertex, in vertex order.
pub struct QuadOut {
    pub coords: [u32; 4],
    pub grey: [u32; 4],
}

/// The blocks and light around the quads a [`Lighter`] lights, at section-local positions.
pub trait LightWorld {
    fn facts(&self, p: [i32; 3]) -> LightFacts;
    /// Block and sky level.
    fn light(&self, p: [i32; 3]) -> (u8, u8);
}

/// A section's blocks and the light gathered around it.
#[derive(Clone, Copy)]
pub(super) struct SectionLight<'a> {
    pub ctx: &'a Ctx<'a>,
    pub cells: &'a LightCells,
}

impl LightWorld for SectionLight<'_> {
    fn facts(&self, p: [i32; 3]) -> LightFacts {
        self.ctx.block(p).light
    }

    fn light(&self, p: [i32; 3]) -> (u8, u8) {
        self.cells.at(p)
    }
}

pub struct Lighter<W> {
    pub world: W,
    /// `CardinalLighting.NETHER` rather than `DEFAULT`.
    pub nether: bool,
}

impl<W: LightWorld> Lighter<W> {
    fn facts(&self, p: [i32; 3]) -> LightFacts {
        self.world.facts(p)
    }

    /// `getShadeBrightness`.
    fn shade(&self, p: [i32; 3]) -> f32 {
        if self.facts(p).dark { 0.2 } else { 1.0 }
    }

    /// `LightCoordsUtil.getLightCoords`: light at `p` as seen through `state`.
    fn coords(&self, state: &LightFacts, p: [i32; 3]) -> u32 {
        if state.emissive {
            return FULL_BRIGHT;
        }
        let (block, sky) = self.world.light(p);
        u32::from(block.max(state.emission)) << 4 | u32::from(sky) << 20
    }

    pub fn light(&self, q: &QuadIn) -> QuadOut {
        let directional = cardinal(self.nether, q.shade);
        let own = &self.facts(q.pos);
        let facing = facing(&q.positions);
        if q.ambient_occlusion && own.emission == 0 {
            let (brightness, coords) = self.ambient_occlusion(own, q.pos, &q.positions, facing);
            return QuadOut { coords, grey: brightness.map(|b| scale(grey(b), directional)) };
        }
        let coords = match q.cull {
            // The block's own state at its neighbour's light (`LevelRenderer.getLightCoords`).
            Some(d) => self.coords(own, step(q.pos, d)),
            None => {
                let at = if shape(&q.positions, facing, own.full_collision).cubic { step(q.pos, facing) } else { q.pos };
                self.coords(own, at)
            }
        };
        QuadOut { coords: [coords; 4], grey: [grey(directional) as u32; 4] }
    }

    /// `prepareQuadAmbientOcclusion`: brightness before `ARGB.gray` and smooth light coords.
    fn ambient_occlusion(&self, own: &LightFacts, pos: [i32; 3], positions: &[[f32; 3]; 4], face: Dir) -> ([f32; 4], [u32; 4]) {
        let shape = shape(positions, face, own.full_collision);
        let base = if shape.cubic { step(pos, face) } else { pos };
        let adjacency = &ADJACENCY[usize::from(face)];
        let corner = |i: usize| step(base, adjacency.corners[i]);
        let sample = |p: [i32; 3]| (self.coords(&self.facts(p), p), self.shade(p));
        let [(light0, shade0), (light1, shade1), (light2, shade2), (light3, shade3)] = [0, 1, 2, 3].map(|i| sample(corner(i)));
        let permeable = |i: usize| self.facts(step(corner(i), face)).permeable();
        let (p0, p1, p2, p3) = (permeable(0), permeable(1), permeable(2), permeable(3));
        // Neither side open: Java reuses corner 0 for the diagonal, whichever diagonal it is.
        let diagonal = |a: usize, b: usize, open: bool| if open { sample(step(corner(a), adjacency.corners[b])) } else { (light0, shade0) };
        let (light02, shade02) = diagonal(0, 2, p2 || p0);
        let (light03, shade03) = diagonal(0, 3, p3 || p0);
        let (light12, shade12) = diagonal(1, 2, p2 || p1);
        let (light13, shade13) = diagonal(1, 3, p3 || p1);
        let next = step(pos, face);
        let next_state = self.facts(next);
        let light_center = if shape.cubic || !next_state.solid_render { self.coords(&next_state, next) } else { self.coords(own, pos) };
        let shade_center = if shape.cubic { self.shade(base) } else { self.shade(pos) };
        let ao = [
            (shade3 + shade0 + shade03 + shade_center) * 0.25,
            (shade2 + shade0 + shade02 + shade_center) * 0.25,
            (shade2 + shade1 + shade12 + shade_center) * 0.25,
            (shade3 + shade1 + shade13 + shade_center) * 0.25,
        ];
        let blended = [
            smooth_blend(light3, light0, light03, light_center),
            smooth_blend(light2, light0, light02, light_center),
            smooth_blend(light2, light1, light12, light_center),
            smooth_blend(light3, light1, light13, light_center),
        ];
        let (mut brightness, mut light) = ([0f32; 4], [0u32; 4]);
        for (k, &vertex) in adjacency.remap.iter().enumerate() {
            if shape.partial {
                let f = |i: usize| shape.bounds[usize::from(adjacency.weights[k][i])];
                let w = [f(0) * f(1), f(2) * f(3), f(4) * f(5), f(6) * f(7)];
                brightness[vertex] = (ao[0] * w[0] + ao[1] * w[1] + ao[2] * w[2] + ao[3] * w[3]).clamp(0.0, 1.0);
                light[vertex] = weighted_blend(blended, w);
            } else {
                brightness[vertex] = ao[k];
                light[vertex] = blended[k];
            }
        }
        (brightness, light)
    }
}

/// `LightCoordsUtil.smoothBlend`: zero samples take the centre; samples without sky light borrow
/// the centre's sky.
pub(super) fn smooth_blend(mut a: u32, mut b: u32, mut c: u32, center: u32) -> u32 {
    if center >> 20 & 15 > 2 || center >> 4 & 15 > 2 {
        for n in [&mut a, &mut b, &mut c] {
            if *n == 0 {
                *n = center;
            } else if *n >> 20 & 15 == 0 {
                *n |= center & 0x00FF_0000;
            }
        }
    }
    (a + b + c + center) >> 2 & 0x00FF_00FF
}

/// `LightCoordsUtil.smoothWeightedBlend`: Java's float sums and int narrowing.
fn weighted_blend(c: [u32; 4], w: [f32; 4]) -> u32 {
    let channel = |shift: u32| {
        let v = |i: usize| (c[i] >> shift & 0xFF) as f32 * w[i];
        (v(0) + v(1) + v(2) + v(3)) as i32 as u32 & 0xFF
    };
    channel(0) | channel(16) << 16
}

/// `ARGB.gray`'s channel: `Mth.floor(brightness * 255)`.
pub(super) fn grey(brightness: f32) -> i32 {
    (brightness * 255.0).floor() as i32
}

/// One channel of `ARGB.scaleRGB(color, float)`.
pub(super) fn scale(channel: i32, by: f32) -> u32 {
    ((channel as f32 * by) as i32).clamp(0, 255) as u32
}

/// `prepareQuadShape`: the quad's bounds (and one minus each), whether it is smaller than the
/// block face, and whether it is flush with the block's side (or flat on a full collision shape).
pub(super) struct Shape {
    pub bounds: [f32; 12],
    pub partial: bool,
    pub cubic: bool,
}

pub(super) fn shape(positions: &[[f32; 3]; 4], face: Dir, full: bool) -> Shape {
    let (mut min, mut max) = ([32f32; 3], [-32f32; 3]);
    for p in positions {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    let mut bounds = [0f32; 12];
    for (index, value) in [(WEST, min[0]), (EAST, max[0]), (DOWN, min[1]), (UP, max[1]), (NORTH, min[2]), (SOUTH, max[2])] {
        bounds[usize::from(index)] = value;
        bounds[usize::from(index) + 6] = 1.0 - value;
    }
    let (lo, hi) = (1.0e-4f32, 0.9999f32);
    let partial = |a: usize, b: usize| min[a] >= lo || min[b] >= lo || max[a] <= hi || max[b] <= hi;
    let (partial, cubic) = match face {
        DOWN => (partial(0, 2), min[1] == max[1] && (min[1] < lo || full)),
        UP => (partial(0, 2), min[1] == max[1] && (max[1] > hi || full)),
        NORTH => (partial(0, 1), min[2] == max[2] && (min[2] < lo || full)),
        SOUTH => (partial(0, 1), min[2] == max[2] && (max[2] > hi || full)),
        WEST => (partial(1, 2), min[0] == max[0] && (min[0] < lo || full)),
        _ => (partial(1, 2), min[0] == max[0] && (max[0] > hi || full)),
    };
    Shape { bounds, partial, cubic }
}
