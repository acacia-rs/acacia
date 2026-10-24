//! The volume a [`Lighter`] fills and floods: cells from block states, sky and block light seeds, and
//! Java's bucketed propagation.

use super::{BLOCK, FACE, Lighter, OPAQUE, SKY};
use crate::column::JavaColumn;
use crate::props::{self, DOWN, EAST, NORTH, PropsTable, SOUTH, StateProps, UP, WEST};
use crate::section::Blocks;

fn cell_of(p: StateProps) -> u32 {
    p.opacity as u32 | (p.faces as u32) << FACE
}

impl Lighter {
    pub(super) fn fill(&mut self, cols: &[Option<&JavaColumn>], props: &PropsTable) {
        let (pl, px, cols_x) = (self.pl, self.px, self.cols_x);
        self.vox.clear();
        self.vox.resize(self.cells(), OPAQUE);
        self.emitters.clear();
        for (i, col) in cols.iter().enumerate() {
            let Some(col) = col else { continue };
            let (ox, oz) = ((i % cols_x) * 16, (i / cols_x) * 16);
            for (s, blocks) in col.sections.iter().enumerate().take(self.lit) {
                let base = self.at(ox, s * 16, oz);
                let rows = (0..256).map(|r| (r << 4, base + (r >> 4) * pl + (r & 15) * px));
                match blocks {
                    Blocks::Uniform(u) => {
                        let p = props.get(*u);
                        for (_, d) in rows.clone() {
                            self.vox[d..d + 16].fill(cell_of(p));
                        }
                        if p.emission > 0 {
                            for (_, d) in rows {
                                self.emitters.extend((d..d + 16).map(|d| (d as u32, p.emission)));
                            }
                        }
                    }
                    Blocks::Small { palette, idx } => {
                        let mut pal = [0u32; 256];
                        for (c, &st) in pal.iter_mut().zip(palette) {
                            *c = cell_of(props.get(st));
                        }
                        for (src, d) in rows.clone() {
                            let row = &mut self.vox[d..d + 16];
                            for (c, &i) in row.iter_mut().zip(&idx[src..src + 16]) {
                                *c = pal[i as usize];
                            }
                        }
                        if palette.iter().any(|&st| props.get(st).emission > 0) {
                            for (src, d) in rows {
                                for x in 0..16 {
                                    let e = props.get(palette[idx[src | x] as usize]).emission;
                                    if e > 0 {
                                        self.emitters.push(((d + x) as u32, e));
                                    }
                                }
                            }
                        }
                    }
                    Blocks::Full(f) => {
                        for (src, d) in rows {
                            for x in 0..16 {
                                let p = props.get(f[src | x]);
                                self.vox[d + x] = cell_of(p);
                                if p.emission > 0 {
                                    self.emitters.push(((d + x) as u32, p.emission));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Straight-down 15s per column, then queue the ones next to a darker column and each streak's end.
    pub(super) fn seed_sky(&mut self) {
        let (pl, px, h) = (self.pl, self.px, self.height);
        // Lowest lit row per column (bordered y), h + 1 for none.
        self.bottom.clear();
        self.bottom.resize(pl, (h + 1) as u32);
        self.open.clear();
        self.open.resize(pl, 1);
        let (blocks, down) = (0xF | 1 << (FACE + UP as u32), 1 << (FACE + DOWN as u32));
        // Layer by layer from the top, so the walk reads memory in order; stops once every column is shut.
        let mut y = h + 1;
        while y > 1 {
            y -= 1;
            let row = &mut self.vox[y * pl..(y + 1) * pl];
            let mut any = 0;
            for ((c, op), bt) in row.iter_mut().zip(&mut self.open).zip(&mut self.bottom) {
                let lit = *op & (*c & blocks == 0) as u8;
                *c |= (lit as u32 * 15) << SKY;
                *bt = if lit != 0 { y as u32 } else { *bt };
                *op = lit & (*c & down == 0) as u8;
                any |= *op;
            }
            if any == 0 {
                break;
            }
        }
        // The border never lights its neighbours.
        let pz = pl / px;
        for x in 0..px {
            self.bottom[x] = 0;
            self.bottom[(pz - 1) * px + x] = 0;
        }
        for z in 0..pz {
            self.bottom[z * px] = 0;
            self.bottom[z * px + px - 1] = 0;
        }
        for z in 1..pz - 1 {
            for xz in z * px + 1..z * px + px - 1 {
                let b = self.bottom[xz];
                let reach = [xz - 1, xz + 1, xz - px, xz + px]
                    .iter()
                    .fold(b + 1, |r, &n| r.max(self.bottom[n]));
                for y in b as usize..(reach as usize).min(h + 1) {
                    let d = y * pl + xz;
                    if self.vox[d] >> SKY & 15 == 15 {
                        self.buckets[15].push(d as u32);
                    }
                }
            }
        }
    }

    pub(super) fn seed_block(&mut self) {
        for &(d, e) in &self.emitters {
            self.vox[d as usize] |= (e as u32) << BLOCK;
            self.buckets[e as usize].push(d);
        }
    }

    /// Floods the level at bit `S` from the queued cells, highest first. A cell at 1 lights nothing, so
    /// none is queued.
    pub(super) fn propagate<const S: u32, const SKY_RULE: bool>(&mut self) {
        let (pl, px) = (self.pl as isize, self.px as isize);
        for l in (2..16).rev() {
            while let Some(d) = self.buckets[l as usize].pop() {
                let d = d as usize;
                let c = self.vox[d];
                if c >> S & 15 != l {
                    continue;
                }
                self.step::<S, SKY_RULE>(c, d, -pl, DOWN, l);
                self.step::<S, SKY_RULE>(c, d, pl, UP, l);
                self.step::<S, SKY_RULE>(c, d, -px, NORTH, l);
                self.step::<S, SKY_RULE>(c, d, px, SOUTH, l);
                self.step::<S, SKY_RULE>(c, d, -1, WEST, l);
                self.step::<S, SKY_RULE>(c, d, 1, EAST, l);
            }
        }
        self.buckets[1].clear();
    }

    /// Light `l` from cell `from` at `d` into its neighbour `d + off` in direction `dir`.
    #[inline(always)]
    fn step<const S: u32, const SKY_RULE: bool>(
        &mut self,
        from: u32,
        d: usize,
        off: isize,
        dir: usize,
        l: u32,
    ) {
        if from & 1 << (FACE as usize + dir) != 0 {
            return;
        }
        let nb = d.wrapping_add_signed(off);
        let c = self.vox[nb];
        if c & 1 << (FACE as usize + props::opposite(dir)) != 0 {
            return;
        }
        let o = c & 15;
        let nl = if SKY_RULE && l == 15 && o == 0 && dir == DOWN {
            15
        } else {
            l.saturating_sub(o.max(1))
        };
        if nl > c >> S & 15 {
            self.vox[nb] = c & !(15 << S) | nl << S;
            if nl > 1 {
                self.buckets[nl as usize].push(nb as u32);
            }
        }
    }
}
