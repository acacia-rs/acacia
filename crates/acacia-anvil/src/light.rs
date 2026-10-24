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

/// A cell, one word so a step loads one cache line: dampening in bits 0–3, full occluding faces from bit
/// `FACE` (one per direction), sky level at bit `SKY`, block level at bit `BLOCK`.
const OPAQUE: u32 = 15;
const FACE: u32 = 4;
const SKY: u32 = 16;
const BLOCK: u32 = 24;

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

/// Per-thread buffers over one lit volume: `cols_x × cols_z` columns, `height` blocks from the bottom,
/// wrapped in a one-cell opaque border so propagation never needs coordinates or bounds.
#[derive(Default)]
pub struct Lighter {
    /// Volume size in blocks along x and z (inside the border), and its height.
    sx: usize,
    sz: usize,
    height: usize,
    /// Row and layer strides of the bordered volume: `(sx + 2)` and `(sx + 2) × (sz + 2)`.
    px: usize,
    pl: usize,
    /// Sections lit (from the bottom); the ones above are open sky.
    lit: usize,
    has_sky: bool,
    cols_x: usize,
    vox: Vec<u32>,
    emitters: Vec<(u32, u8)>,
    buckets: [Vec<u32>; 16],
    bottom: Vec<u32>,
    open: Vec<u8>,
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
        self.px = self.sx + 2;
        self.pl = self.px * (self.sz + 2);
        self.has_sky = has_sky;
        self.cols_x = cols_x;
        self.fill(cols, props);
        if has_sky && self.height > 0 {
            self.seed_sky();
            self.propagate::<SKY, true>();
        }
        self.seed_block();
        self.propagate::<BLOCK, false>();
    }

    /// Cells in the bordered volume.
    fn cells(&self) -> usize {
        self.pl * (self.height + 2)
    }

    /// Index of block (`x`, `y`, `z`) of the volume.
    fn at(&self, x: usize, y: usize, z: usize) -> usize {
        (y + 1) * self.pl + (z + 1) * self.px + x + 1
    }

    /// The light of column (`cx`, `cz`) of the last [`Self::light_area`], `sections` high.
    pub fn extract(&self, cx: usize, cz: usize, sections: usize) -> Vec<SectionLight> {
        let mut out = Vec::new();
        self.extract_into(cx, cz, sections, &mut out);
        out
    }

    /// [`Self::extract`] into `out`, reusing its arrays.
    pub fn extract_into(&self, cx: usize, cz: usize, sections: usize, out: &mut Vec<SectionLight>) {
        let sky_byte = if self.has_sky { 0xFF } else { 0 };
        let (ox, oz) = (cx * 16, cz * 16);
        out.truncate(sections);
        out.resize_with(sections, || SectionLight {
            sky: Box::new([0; 2048]),
            block: Box::new([0; 2048]),
        });
        for (s, light) in out.iter_mut().enumerate() {
            if s >= self.lit {
                light.sky.fill(sky_byte);
                light.block.fill(0);
                continue;
            }
            for y in 0..16 {
                for z in 0..16 {
                    let d = self.at(ox, s * 16 + y, oz + z);
                    let o = (y << 7) | (z << 3);
                    let row: &[u32; 16] = self.vox[d..d + 16].try_into().unwrap();
                    let sky: &mut [u8; 8] = (&mut light.sky[o..o + 8]).try_into().unwrap();
                    let block: &mut [u8; 8] = (&mut light.block[o..o + 8]).try_into().unwrap();
                    pack_row(row, sky, block);
                }
            }
        }
    }
}

/// 16 levels to 8 nibble pairs, even x in the low nibble.
#[inline]
fn pack_row(cells: &[u32; 16], sky: &mut [u8; 8], block: &mut [u8; 8]) {
    const LEVELS: u32 = 15 << SKY | 15 << BLOCK;
    for x in 0..8 {
        // Odd x's levels land in the high nibbles: sky in bits 16-23, block in 24-31.
        let t = cells[2 * x] & LEVELS | (cells[2 * x + 1] & LEVELS) << 4;
        sky[x] = (t >> SKY) as u8;
        block[x] = (t >> BLOCK) as u8;
    }
}

#[cfg(test)]
mod tests;
