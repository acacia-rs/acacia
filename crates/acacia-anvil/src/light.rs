//! Sky and block light for one column from the blocks of its 3×3 neighbourhood.
//!
//! Light reaches at most 15 blocks, so every path that lights the centre column stays inside the 16-block
//! ring of neighbours: with all eight present the result is exact (for our opacity model, `props.rs`).
//! A missing neighbour counts as opaque, so light only stops at it; the ingest loop relights a column when
//! a late neighbour arrives (`store.rs`).

use crate::column::JavaColumn;
use crate::props::{self, PropsTable};
use crate::section::{Blocks, index};

const SIDE: usize = 48;
const LAYER: usize = SIDE * SIDE;

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
}

#[derive(Default)]
pub struct Lighter {
    opacity: Vec<u8>,
    faces: Vec<u8>,
    emission: Vec<u8>,
    level: Vec<u8>,
    buckets: [Vec<u32>; 16],
    bottom: Vec<u32>,
}

impl Lighter {
    /// Light for every section of the centre column.
    pub fn light(
        &mut self,
        n: &Neighbourhood,
        props: &PropsTable,
        has_sky: bool,
    ) -> Vec<SectionLight> {
        let c = n.centre();
        let sections = c.sections.len();
        let top_section = n
            .cols
            .iter()
            .flatten()
            .filter_map(|col| {
                col.sections
                    .iter()
                    .rposition(|s| s.uniform() != Some(crate::mapping::AIR))
            })
            .max();
        // Above the highest non-air section anywhere (plus one for light rising off it) is open sky.
        let lit = top_section.map_or(0, |t| (t + 2).min(sections));
        let height = lit * 16;
        self.fill(n, props, height);

        let mut out = Vec::with_capacity(sections);
        let sky_byte = if has_sky { 0xFF } else { 0 };
        let mut sky = vec![0u8; LAYER * height];
        if has_sky && height > 0 {
            self.seed_sky(height);
            self.propagate(true);
            sky.copy_from_slice(&self.level);
        }
        self.seed_block(height);
        self.propagate(false);
        for s in 0..sections {
            let mut light = SectionLight {
                sky: Box::new([sky_byte; 2048]),
                block: Box::new([0; 2048]),
            };
            if s < lit {
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let d = (s * 16 + y) * LAYER + (z + 16) * SIDE + x + 16;
                            set_nibble(&mut light.sky, index(x, y, z), sky[d]);
                            set_nibble(&mut light.block, index(x, y, z), self.level[d]);
                        }
                    }
                }
            }
            out.push(light);
        }
        out
    }

    fn fill(&mut self, n: &Neighbourhood, props: &PropsTable, height: usize) {
        let cells = LAYER * height;
        for v in [&mut self.opacity, &mut self.faces, &mut self.emission] {
            v.clear();
            v.resize(cells, 0);
        }
        self.opacity.fill(15);
        for (i, col) in n.cols.iter().enumerate() {
            let Some(col) = col else { continue };
            let (ox, oz) = ((i % 3) * 16, (i / 3) * 16);
            for (s, blocks) in col.sections.iter().enumerate().take(height / 16) {
                for y in 0..16 {
                    let row = (s * 16 + y) * LAYER;
                    for z in 0..16 {
                        for x in 0..16 {
                            let p = props.get(match blocks {
                                Blocks::Uniform(u) => *u,
                                b => b.get(index(x, y, z)),
                            });
                            let d = row + (oz + z) * SIDE + ox + x;
                            self.opacity[d] = p.opacity;
                            self.faces[d] = p.faces;
                            self.emission[d] = p.emission;
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
                if self.opacity[d] != 0 || self.faces[d] & (1 << props::UP) != 0 {
                    break;
                }
                self.level[d] = 15;
                y -= 1;
                if self.faces[d] & (1 << props::DOWN) != 0 {
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
        for (d, &e) in self.emission.iter().enumerate() {
            if e > 0 {
                self.level[d] = e;
                self.buckets[e as usize].push(d as u32);
            }
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
                let steps = [
                    (props::DOWN, y > 0, d.wrapping_sub(LAYER)),
                    (props::UP, y + 1 < height, d + LAYER),
                    (props::NORTH, z > 0, d.wrapping_sub(SIDE)),
                    (props::SOUTH, z + 1 < SIDE, d + SIDE),
                    (props::WEST, x > 0, d.wrapping_sub(1)),
                    (props::EAST, x + 1 < SIDE, d + 1),
                ];
                for (dir, inside, nb) in steps {
                    if !inside
                        || self.faces[d] & (1 << dir) != 0
                        || self.faces[nb] & (1 << props::opposite(dir)) != 0
                    {
                        continue;
                    }
                    let o = self.opacity[nb];
                    let nl = if sky && l == 15 && o == 0 && dir == props::DOWN {
                        15
                    } else {
                        l.saturating_sub(o.max(1) as usize)
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

#[inline]
fn set_nibble(arr: &mut [u8; 2048], i: usize, v: u8) {
    let b = &mut arr[i >> 1];
    *b = if i & 1 == 0 {
        (*b & 0xF0) | v
    } else {
        (*b & 0x0F) | (v << 4)
    };
}

#[cfg(test)]
mod tests;
