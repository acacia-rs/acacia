//! Terrain heights a flight keeps clear of: the top non-air block of each column.

use std::collections::HashMap;
use std::sync::Arc;

use acacia_world::{BlockAccess, BlockRegistry, Chunk, ChunkView, ColumnHeights};

pub trait Surface {
    /// Y of the highest non-air block in column (x, z) (`i32::MIN`: none); `None` while its chunk or any of its
    /// sections is unknown.
    fn top(&mut self, x: i32, z: i32) -> Option<i32>;

    /// The column's top block is a floor to land on: solid, no liquid, nothing that hurts.
    fn landable(&mut self, x: i32, z: i32) -> bool;
}

/// Floors a landing must not end on, by name fragment.
const HURTS: [&str; 7] = ["lava", "magma", "fire", "cactus", "sweet_berry", "powder_snow", "pointed_dripstone"];

/// Column heights by chunk, kept for a flight; a chunk is redone when its slot is replaced (re-sent).
#[derive(Default)]
pub struct TopCache {
    chunks: HashMap<(i32, i32), (usize, Option<(i32, Box<ColumnHeights>)>)>,
}

/// The live [`Surface`]: a chunk view and the flight's cache.
pub struct ChunkSurface<'a> {
    pub view: &'a ChunkView,
    pub registry: &'a BlockRegistry,
    pub cache: &'a mut TopCache,
}

impl Surface for ChunkSurface<'_> {
    fn top(&mut self, x: i32, z: i32) -> Option<i32> {
        let (cx, cz) = (x >> 4, z >> 4);
        let chunk = self.view.chunk(cx, cz)?;
        let key = Arc::as_ptr(chunk) as usize;
        let entry = self.cache.chunks.entry((cx, cz)).or_insert((0, None));
        // An unknown chunk is retried each ask: its sections may have arrived since.
        if entry.0 != key || entry.1.is_none() {
            *entry = (key, heights(&chunk.read()));
        }
        let (min_y, heights) = entry.1.as_ref()?;
        let above = heights.get(x, z);
        Some(if above == *min_y { i32::MIN } else { above - 1 })
    }

    fn landable(&mut self, x: i32, z: i32) -> bool {
        let Some(y) = self.top(x, z).filter(|&y| y != i32::MIN) else { return false };
        let ids = (self.view.block(x, y, z), self.view.liquid(x, y, z));
        let (Some(s), Some(l)) = (self.registry.get(ids.0), self.registry.get(ids.1)) else { return false };
        s.is_solid() && !s.is_liquid() && l.is_air() && !HURTS.iter().any(|h| s.name.contains(h))
    }
}

/// `None` while a section is unknown: unknown sections read as air.
fn heights(chunk: &Chunk) -> Option<(i32, Box<ColumnHeights>)> {
    let dim = chunk.dimension();
    (0..chunk.section_count()).all(|i| chunk.section_known(i)).then(|| (dim.min_y, Box::new(chunk.column_heights(&|id| id != dim.air))))
}

/// Blocks between corridor samples: narrower than any spire worth flying around is wide, at cruise height.
const CORRIDOR_STEP: f32 = 4.0;

/// Highest known top along the segment from `a` to `b` (xz), `width` blocks either side, and whether every
/// sampled column was known.
pub(super) fn corridor_top(s: &mut dyn Surface, a: [f32; 2], b: [f32; 2], width: f32) -> (i32, bool) {
    corridor(s, a, b, width).into_iter().fold((i32::MIN, true), |(top, known), (_, y)| match y {
        Some(y) => (top.max(y), known),
        None => (top, false),
    })
}

/// Every corridor sample from `a` to `b`: blocks from `a`, and the column's top (`None` while unknown).
pub(super) fn corridor(s: &mut dyn Surface, a: [f32; 2], b: [f32; 2], width: f32) -> Vec<(f32, Option<i32>)> {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dz);
    let (ux, uz) = if len > 0.0 { (dx / len, dz / len) } else { (0.0, 0.0) };
    let mut out = Vec::with_capacity(3 * (len / CORRIDOR_STEP) as usize + 3);
    let mut t = 0.0;
    while t <= len {
        for side in [-width, 0.0, width] {
            let (x, z) = (a[0] + ux * t - uz * side, a[1] + uz * t + ux * side);
            out.push((t, s.top(x.floor() as i32, z.floor() as i32)));
        }
        t += CORRIDOR_STEP;
    }
    out
}

/// Blocks along `dir` (unit, xz) from `from` to the first column not yet known, up to `max`.
pub(super) fn known_ahead(s: &mut dyn Surface, from: [f32; 2], dir: [f32; 2], max: f32) -> f32 {
    let mut t = 0.0;
    while t < max {
        if s.top((from[0] + dir[0] * t).floor() as i32, (from[1] + dir[1] * t).floor() as i32).is_none() {
            return t;
        }
        t += 8.0;
    }
    max
}
