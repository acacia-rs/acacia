//! Sky and block light from the blocks around a column, with Java's rules (`LightEngine`): each step
//! costs `max(1, dampening)`, sky light keeps 15 straight down through undampened blocks, and
//! shape-occluding blocks pass no light through their full faces (props.rs).
//!
//! Light reaches at most 15 blocks sideways, so every path that lights a column stays inside the 16-block
//! ring of its neighbours: a column is exact once its 3×3 neighbourhood is in the lit volume, whatever
//! else the volume holds. So the store lights a whole group of columns over one padded volume
//! ([`Lighter::light_area`]), which matches lighting each over its own 3×3 ([`Lighter::light`]). A missing
//! column counts as opaque, so light only stops at it; the store relights a column when a late neighbour
//! arrives (`store.rs`).

mod engine;

use crate::column::JavaColumn;
use crate::mapping::{AIR, JavaState};
use crate::props::PropsTable;

/// A cell: dampening in the low byte, full occluding faces in the high byte.
const OPAQUE: u16 = 15;

/// Nibble arrays of one section, in Java's layout (`(y << 8) | (z << 4) | x`, even index low nibble).
#[derive(Clone)]
pub struct SectionLight {
    pub sky: Box<[u8; 2048]>,
    pub block: Box<[u8; 2048]>,
}

/// The 3×3 columns around a centre, `cols[(dz + 1) * 3 + (dx + 1)]`; the centre must be present.
pub struct Neighbourhood<'a> {
    pub cols: [Option<&'a JavaColumn>; 9],
}

impl<'a> Neighbourhood<'a> {
    pub fn centre(&self) -> &'a JavaColumn {
        self.cols[4].expect("centre column")
    }

    /// A block at x/z relative to the centre column (−16..32) and world y; missing columns read as air.
    pub fn block(&self, x: i32, y: i32, z: i32) -> JavaState {
        let (cx, cz) = ((x >> 4) + 1, (z >> 4) + 1);
        if !(0..3).contains(&cx) || !(0..3).contains(&cz) {
            return AIR;
        }
        self.cols[(cz * 3 + cx) as usize]
            .map_or(AIR, |c| c.block((x & 15) as usize, y, (z & 15) as usize))
    }
}

/// Per-thread buffers over one lit volume: `cols_x × cols_z` columns, `height` blocks from the bottom.
#[derive(Default)]
pub struct Lighter {
    /// Volume size in blocks along x and z, and its x·z layer.
    sx: usize,
    sz: usize,
    layer: usize,
    height: usize,
    /// Sections lit (from the bottom); the ones above are open sky.
    lit: usize,
    has_sky: bool,
    cell: Vec<u16>,
    level: Vec<u8>,
    sky: Vec<u8>,
    emitters: Vec<(u32, u8)>,
    buckets: [Vec<u32>; 16],
    bottom: Vec<u32>,
    palette: Vec<(u16, u8)>,
}

impl Lighter {
    /// Light for every section of the centre column.
    pub fn light(
        &mut self,
        n: &Neighbourhood,
        props: &PropsTable,
        has_sky: bool,
    ) -> Vec<SectionLight> {
        self.light_area(&n.cols, 3, props, has_sky);
        self.extract(1, 1, n.centre().sections.len())
    }

    /// Lights the columns `cols[z * cols_x + x]` (a rectangle `cols_x` wide) together; each column whose
    /// 3×3 neighbourhood is inside the rectangle then reads its exact light with [`Self::extract`].
    pub fn light_area(
        &mut self,
        cols: &[Option<&JavaColumn>],
        cols_x: usize,
        props: &PropsTable,
        has_sky: bool,
    ) {
        let sections = cols.iter().flatten().map(|c| c.sections.len()).max();
        let top_section = cols
            .iter()
            .flatten()
            .filter_map(|col| col.sections.iter().rposition(|s| s.uniform() != Some(AIR)))
            .max();
        // Above the highest non-air section anywhere (plus one for light rising off it) is open sky.
        self.lit = top_section.map_or(0, |t| (t + 2).min(sections.unwrap_or(0)));
        self.height = self.lit * 16;
        self.sx = cols_x * 16;
        self.sz = cols.len().div_ceil(cols_x) * 16;
        self.layer = self.sx * self.sz;
        self.has_sky = has_sky;
        self.fill(cols, cols_x, props);
        self.sky.clear();
        self.sky.resize(self.layer * self.height, 0);
        if has_sky && self.height > 0 {
            self.seed_sky();
            self.propagate(true);
            std::mem::swap(&mut self.sky, &mut self.level);
        }
        self.seed_block();
        self.propagate(false);
    }

    /// The light of column (`cx`, `cz`) of the last [`Self::light_area`], `sections` high.
    pub fn extract(&self, cx: usize, cz: usize, sections: usize) -> Vec<SectionLight> {
        let sky_byte = if self.has_sky { 0xFF } else { 0 };
        let (ox, oz) = (cx * 16, cz * 16);
        let mut out = Vec::with_capacity(sections);
        for s in 0..sections {
            let mut light = SectionLight {
                sky: Box::new([sky_byte; 2048]),
                block: Box::new([0; 2048]),
            };
            if s < self.lit {
                for y in 0..16 {
                    for z in 0..16 {
                        let d = (s * 16 + y) * self.layer + (oz + z) * self.sx + ox;
                        let (sky, block) = (&self.sky[d..d + 16], &self.level[d..d + 16]);
                        let o = (y << 7) | (z << 3);
                        for x in 0..8 {
                            light.sky[o + x] = sky[2 * x] | (sky[2 * x + 1] << 4);
                            light.block[o + x] = block[2 * x] | (block[2 * x + 1] << 4);
                        }
                    }
                }
            }
            out.push(light);
        }
        out
    }
}

#[cfg(test)]
mod tests;
