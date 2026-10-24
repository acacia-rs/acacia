//! The volume a [`Lighter`] fills and floods: cells from block states, sky and block light seeds, and
//! Java's bucketed propagation.

use super::{Lighter, OPAQUE};
use crate::column::JavaColumn;
use crate::props::{self, PropsTable};
use crate::section::Blocks;

impl Lighter {
    pub(super) fn fill(
        &mut self,
        cols: &[Option<&JavaColumn>],
        cols_x: usize,
        props: &PropsTable,
    ) {
        let (layer, sx, height) = (self.layer, self.sx, self.height);
        self.cell.clear();
        self.cell.resize(layer * height, OPAQUE);
        self.emitters.clear();
        let cell_of = |s| {
            let p = props.get(s);
            (p.opacity as u16 | (p.faces as u16) << 8, p.emission)
        };
        for (i, col) in cols.iter().enumerate() {
            let Some(col) = col else { continue };
            let (ox, oz) = ((i % cols_x) * 16, (i / cols_x) * 16);
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
                        let dst = (s * 16 + y) * layer + (oz + z) * sx + ox;
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
    pub(super) fn seed_sky(&mut self) {
        let (layer, sx, sz, height) = (self.layer, self.sx, self.sz, self.height);
        self.level.clear();
        self.level.resize(layer * height, 0);
        self.bottom.clear();
        self.bottom.resize(layer, 0);
        for xz in 0..layer {
            let mut y = height;
            while y > 0 {
                let d = (y - 1) * layer + xz;
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
        for xz in 0..layer {
            let (x, z) = (xz % sx, xz / sx);
            let mut reach = self.bottom[xz] + 1;
            for (dx, dz) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                if (0..sx as i32).contains(&nx) && (0..sz as i32).contains(&nz) {
                    reach = reach.max(self.bottom[nz as usize * sx + nx as usize]);
                }
            }
            for y in self.bottom[xz] as usize..(reach as usize).min(height) {
                let d = y * layer + xz;
                if self.level[d] == 15 {
                    self.buckets[15].push(d as u32);
                }
            }
        }
    }

    pub(super) fn seed_block(&mut self) {
        self.level.clear();
        self.level.resize(self.layer * self.height, 0);
        for &(d, e) in &self.emitters {
            self.level[d as usize] = e;
            self.buckets[e as usize].push(d);
        }
    }

    pub(super) fn propagate(&mut self, sky: bool) {
        let (layer, sx, sz, height) = (self.layer, self.sx, self.sz, self.height);
        for l in (1..16).rev() {
            while let Some(d) = self.buckets[l].pop() {
                let d = d as usize;
                if self.level[d] as usize != l {
                    continue;
                }
                let (x, z, y) = (d % sx, (d / sx) % sz, d / layer);
                let faces = self.cell[d] >> 8;
                let steps = [
                    (props::DOWN, y > 0, d.wrapping_sub(layer)),
                    (props::UP, y + 1 < height, d + layer),
                    (props::NORTH, z > 0, d.wrapping_sub(sx)),
                    (props::SOUTH, z + 1 < sz, d + sx),
                    (props::WEST, x > 0, d.wrapping_sub(1)),
                    (props::EAST, x + 1 < sx, d + 1),
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
