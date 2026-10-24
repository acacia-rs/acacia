//! Sky and block light for one column from the blocks of its 3×3 neighbourhood, with Java's rules
//! (`LightEngine`): each step costs `max(1, dampening)`, sky light keeps 15 straight down through
//! undampened blocks, and shape-occluding blocks pass no light through their full faces (props.rs).
//!
//! Light reaches at most 15 blocks, so every path that lights the centre column stays inside the 16-block
//! ring of neighbours: with all eight present the result is exact. A missing neighbour counts as opaque,
//! so light only stops at it; the store relights a column when a late neighbour arrives (`store.rs`).

use crate::column::JavaColumn;
use crate::mapping::{AIR, JavaState};
use crate::props::{self, PropsTable};
use crate::section::Blocks;

const SIDE: usize = 48;
const LAYER: usize = SIDE * SIDE;
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

#[derive(Default)]
pub struct Lighter {
    cell: Vec<u16>,
    level: Vec<u8>,
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
        let sections = n.centre().sections.len();
        let top_section = n
            .cols
            .iter()
            .flatten()
            .filter_map(|col| col.sections.iter().rposition(|s| s.uniform() != Some(AIR)))
            .max();
        // Above the highest non-air section anywhere (plus one for light rising off it) is open sky.
        let lit = top_section.map_or(0, |t| (t + 2).min(sections));
        let height = lit * 16;
        self.fill(n, props, height);

        let sky_byte = if has_sky { 0xFF } else { 0 };
        let mut sky = vec![0u8; LAYER * height];
        if has_sky && height > 0 {
            self.seed_sky(height);
            self.propagate(true);
            sky.copy_from_slice(&self.level);
        }
        self.seed_block(height);
        self.propagate(false);
        let mut out = Vec::with_capacity(sections);
        for s in 0..sections {
            let mut light = SectionLight {
                sky: Box::new([sky_byte; 2048]),
                block: Box::new([0; 2048]),
            };
            if s < lit {
                for i in (0..4096).step_by(2) {
                    let (x, z, y) = (i & 15, (i >> 4) & 15, i >> 8);
                    let d = (s * 16 + y) * LAYER + (z + 16) * SIDE + x + 16;
                    light.sky[i >> 1] = sky[d] | (sky[d + 1] << 4);
                    light.block[i >> 1] = self.level[d] | (self.level[d + 1] << 4);
                }
            }
            out.push(light);
        }
        out
    }

    fn fill(&mut self, n: &Neighbourhood, props: &PropsTable, height: usize) {
        self.cell.clear();
        self.cell.resize(LAYER * height, OPAQUE);
        self.emitters.clear();
        let cell_of = |s: JavaState| {
            let p = props.get(s);
            (p.opacity as u16 | (p.faces as u16) << 8, p.emission)
        };
        for (i, col) in n.cols.iter().enumerate() {
            let Some(col) = col else { continue };
            let (ox, oz) = ((i % 3) * 16, (i / 3) * 16);
            for (s, blocks) in col.sections.iter().enumerate().take(height / 16) {
                self.palette.clear();
                match blocks {
                    Blocks::Uniform(u) => self.palette.push(cell_of(*u)),
                    Blocks::Small { palette, .. } => {
                        self.palette.extend(palette.iter().map(|&s| cell_of(s)))
                    }
                    Blocks::Full(_) => {}
                }
                for y in 0..16 {
                    for z in 0..16 {
                        let src = (y << 8) | (z << 4);
                        let dst = (s * 16 + y) * LAYER + (oz + z) * SIDE + ox;
                        let row = &mut self.cell[dst..dst + 16];
                        for x in 0..16 {
                            let (c, e) = match blocks {
                                Blocks::Uniform(_) => self.palette[0],
                                Blocks::Small { idx, .. } => self.palette[idx[src | x] as usize],
                                Blocks::Full(f) => cell_of(f[src | x]),
                            };
                            row[x] = c;
                            if e > 0 {
                                self.emitters.push(((dst + x) as u32, e));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Straight-down 15s per column, then queue the ones next to a darker column and each streak's end.
    fn seed_sky(&mut self, height: usize) {
        self.level.clear();
        self.level.resize(LAYER * height, 0);
        self.bottom.clear();
        self.bottom.resize(LAYER, 0);
        for xz in 0..LAYER {
            let mut y = height;
            while y > 0 {
                let d = (y - 1) * LAYER + xz;
                let c = self.cell[d];
                if c & 0xFF != 0 || c & (1 << (8 + props::UP)) != 0 {
                    break;
                }
                self.level[d] = 15;
                y -= 1;
                if c & (1 << (8 + props::DOWN)) != 0 {
                    break;
                }
            }
            self.bottom[xz] = y as u32;
        }
        for xz in 0..LAYER {
            let (x, z) = (xz % SIDE, xz / SIDE);
            let mut reach = self.bottom[xz] + 1;
            for (dx, dz) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                if (0..SIDE as i32).contains(&nx) && (0..SIDE as i32).contains(&nz) {
                    reach = reach.max(self.bottom[nz as usize * SIDE + nx as usize]);
                }
            }
            for y in self.bottom[xz] as usize..(reach as usize).min(height) {
                let d = y * LAYER + xz;
                if self.level[d] == 15 {
                    self.buckets[15].push(d as u32);
                }
            }
        }
    }

    fn seed_block(&mut self, height: usize) {
        self.level.clear();
        self.level.resize(LAYER * height, 0);
        for &(d, e) in &self.emitters {
            self.level[d as usize] = e;
            self.buckets[e as usize].push(d);
        }
    }

    fn propagate(&mut self, sky: bool) {
        let height = self.level.len() / LAYER;
        for l in (1..16).rev() {
            while let Some(d) = self.buckets[l].pop() {
                let d = d as usize;
                if self.level[d] as usize != l {
                    continue;
                }
                let (x, z, y) = (d % SIDE, (d / SIDE) % SIDE, d / LAYER);
                let faces = self.cell[d] >> 8;
                let steps = [
                    (props::DOWN, y > 0, d.wrapping_sub(LAYER)),
                    (props::UP, y + 1 < height, d + LAYER),
                    (props::NORTH, z > 0, d.wrapping_sub(SIDE)),
                    (props::SOUTH, z + 1 < SIDE, d + SIDE),
                    (props::WEST, x > 0, d.wrapping_sub(1)),
                    (props::EAST, x + 1 < SIDE, d + 1),
                ];
                for (dir, inside, nb) in steps {
                    if !inside || faces & (1 << dir) != 0 {
                        continue;
                    }
                    let c = self.cell[nb];
                    if (c >> 8) & (1 << props::opposite(dir)) != 0 {
                        continue;
                    }
                    let o = (c & 0xFF) as usize;
                    let nl = if sky && l == 15 && o == 0 && dir == props::DOWN {
                        15
                    } else {
                        l.saturating_sub(o.max(1))
                    };
                    if nl > self.level[nb] as usize {
                        self.level[nb] = nl as u8;
                        self.buckets[nl].push(nb as u32);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
