//! The volume a [`Lighter`] fills and floods: cells from block states, sky and block light seeds, and
//! Java's bucketed propagation.

use super::{Lighter, OPAQUE};
use crate::column::JavaColumn;
use crate::props::{self, PropsTable, StateProps};
use crate::section::Blocks;

fn cell_of(p: StateProps) -> u16 {
    p.opacity as u16 | (p.faces as u16) << 8
}

impl Lighter {
    pub(super) fn fill(
        &mut self,
        cols: &[Option<&JavaColumn>],
        cols_x: usize,
        props: &PropsTable,
    ) {
        let (pl, px) = (self.pl, self.px);
        self.cell.clear();
        self.cell.resize(self.cells(), OPAQUE);
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
                            self.cell[d..d + 16].fill(cell_of(p));
                        }
                        if p.emission > 0 {
                            for (_, d) in rows {
                                self.emitters.extend((d..d + 16).map(|d| (d as u32, p.emission)));
                            }
                        }
                    }
                    Blocks::Small { palette, idx } => {
                        let mut pal = [0u16; 256];
                        for (c, &st) in pal.iter_mut().zip(palette) {
                            *c = cell_of(props.get(st));
                        }
                        for (src, d) in rows.clone() {
                            let row = &mut self.cell[d..d + 16];
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
                                self.cell[d + x] = cell_of(p);
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
        self.level.resize(self.cells(), 0);
        self.level[(h + 1) * pl..].fill(0);
        // Lowest lit row per column (bordered y), h + 1 for none.
        self.bottom.clear();
        self.bottom.resize(pl, (h + 1) as u32);
        self.open.clear();
        self.open.resize(pl, 1);
        let blocks = 0xFF | 1 << (8 + props::UP);
        // Layer by layer from the top, so the walk reads memory in order; stops once every column is shut.
        let mut y = h + 1;
        while y > 1 {
            y -= 1;
            let at = y * pl;
            let (cells, level) = (&self.cell[at..at + pl], &mut self.level[at..at + pl]);
            let mut any = 0;
            let it = cells.iter().zip(level).zip(&mut self.open).zip(&mut self.bottom);
            for (((&c, lv), op), bt) in it {
                let lit = *op & (c & blocks == 0) as u8;
                *lv = lit * 15;
                *bt = if lit != 0 { y as u32 } else { *bt };
                *op = lit & (c & 1 << (8 + props::DOWN) == 0) as u8;
                any |= *op;
            }
            if any == 0 {
                break;
            }
        }
        self.level[..y * pl].fill(0);
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
                    if self.level[d] == 15 {
                        self.buckets[15].push(d as u32);
                    }
                }
            }
        }
    }

    pub(super) fn seed_block(&mut self) {
        self.level.clear();
        self.level.resize(self.cells(), 0);
        for &(d, e) in &self.emitters {
            self.level[d as usize] = e;
            self.buckets[e as usize].push(d);
        }
    }

    /// Floods from the queued cells, highest level first. A cell at 1 lights nothing, so none is queued.
    pub(super) fn propagate<const SKY: bool>(&mut self) {
        let (pl, px) = (self.pl as isize, self.px as isize);
        let steps = [
            (props::DOWN, -pl),
            (props::UP, pl),
            (props::NORTH, -px),
            (props::SOUTH, px),
            (props::WEST, -1),
            (props::EAST, 1),
        ];
        for l in (2..16).rev() {
            while let Some(d) = self.buckets[l].pop() {
                let d = d as usize;
                if self.level[d] as usize != l {
                    continue;
                }
                let faces = self.cell[d] >> 8;
                for (dir, off) in steps {
                    if faces & (1 << dir) != 0 {
                        continue;
                    }
                    let nb = d.wrapping_add_signed(off);
                    let c = self.cell[nb];
                    if (c >> 8) & (1 << props::opposite(dir)) != 0 {
                        continue;
                    }
                    let o = (c & 0xFF) as usize;
                    let nl = if SKY && l == 15 && o == 0 && dir == props::DOWN {
                        15
                    } else {
                        l.saturating_sub(o.max(1))
                    };
                    if nl > self.level[nb] as usize {
                        self.level[nb] = nl as u8;
                        if nl > 1 {
                            self.buckets[nl].push(nb as u32);
                        }
                    }
                }
            }
        }
        self.buckets[1].clear();
    }
}
