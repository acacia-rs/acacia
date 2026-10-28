//! Where sky light comes from. The renderer's own rules let it fall straight down at 15 while
//! nothing filters it. Java's (`ChunkSkyLightSources`) make every block a source from the top of
//! the world down to the first edge that stops light coming straight down ([`Rules::edge_occluded`]).

use std::ops::Range;

use super::blocks::LightBlocks;
use super::column::Lighter;
use super::data::LightData;
use super::propagate::Propagator;

const HORIZONTAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

impl<B: LightBlocks> Lighter<'_, B> {
    /// Fills the straight-down sky columns at 15, then spreads from wherever a neighbour may be
    /// darker: the end of each column and its sides that face a shorter column or the chunk edge.
    pub(super) fn seed_sky(&self, p: &mut Propagator, cx: i32, cz: i32) {
        let (x0, z0, top) = (cx * 16, cz * 16, self.top());
        let mut lowest = [[top; 16]; 16];
        for lx in 0..16 {
            for lz in 0..16 {
                let (x, z) = (x0 + lx, z0 + lz);
                if self.rules.is_java() {
                    let low = p.data.sky_lowest(x, z).unwrap_or(top);
                    (low..top).for_each(|y| p.set([x, y, z], 15));
                    if low > self.bottom() && low < top {
                        p.spread_from([x, low, z]);
                    }
                    lowest[lx as usize][lz as usize] = low;
                    continue;
                }
                let mut level = 15;
                let mut y = top - 1;
                while y >= self.bottom() {
                    let filter = p.data.props.get(x, y, z).map_or(15, |v| v & 15);
                    level = self.rules.step(level, filter, true);
                    if level < 15 {
                        p.raise([x, y, z], level);
                        break;
                    }
                    p.set([x, y, z], 15);
                    lowest[lx as usize][lz as usize] = y;
                    y -= 1;
                }
            }
        }
        for lx in 0..16 {
            for lz in 0..16 {
                let low = lowest[lx as usize][lz as usize];
                for (dx, dz) in HORIZONTAL {
                    let (nx, nz) = (lx + dx, lz + dz);
                    let edge = !(0..16).contains(&nx) || !(0..16).contains(&nz);
                    let neighbour_low = if edge { top } else { lowest[nx as usize][nz as usize] };
                    for y in low..neighbour_low.min(top) {
                        p.spread_from([x0 + lx, y, z0 + lz]);
                    }
                }
            }
        }
    }

    /// Java's rules (`ChunkSkyLightSources.fillFrom`): the lowest source of every block column of
    /// a column, `empty` telling which of [`Lighter::section_range`] hold only air. This scan
    /// stops at the world's bottom block without looking under it, and takes the block over an
    /// empty section for air.
    pub(super) fn find_sky_sources(&self, data: &mut LightData, cx: i32, cz: i32, empty: &[bool]) {
        let first = self.section_range().start;
        let over_empty = |top: i32| top & 15 == 0 && empty.get(((top >> 4) - 1 - first) as usize).copied().unwrap_or(true);
        let mut lowest = Box::new([0; 256]);
        for (i, low) in lowest.iter_mut().enumerate() {
            let (x, z) = (cx * 16 + (i as i32 >> 4), cz * 16 + (i as i32 & 15));
            let edges = (self.dim.min_y + 1..=self.top()).rev();
            *low = edges.filter(|&y| !over_empty(y)).find(|&y| self.edge_occluded(data, x, y, z)).unwrap_or(self.bottom());
        }
        data.sky_sources.insert((cx, cz), lowest);
    }

    /// Java's rules (`ChunkSkyLightSources.update`), after the block at `x`, `y`, `z` changed: the
    /// heights that stopped being sources and those that became sources. Unlike the first scan,
    /// this one does look under the world's bottom block, so the two can disagree.
    pub(super) fn move_sky_sources(&self, data: &mut LightData, x: i32, y: i32, z: i32) -> (Range<i32>, Range<i32>) {
        let Some(old) = data.sky_lowest(x, z) else { return (0..0, 0..0) };
        let below_world = self.dim.min_y - 1;
        let current = if old == self.bottom() { below_world } else { old };
        let edge = |top: i32| -> Option<i32> {
            if self.edge_occluded(data, x, top, z) {
                (top > current).then_some(top)
            } else if top == current {
                Some((below_world + 1..top).rev().find(|&t| self.edge_occluded(data, x, t, z)).unwrap_or(below_world))
            } else {
                None
            }
        };
        let found = if y + 1 < current { None } else { edge(y + 1).or_else(|| edge(y)) };
        let new = match found {
            Some(v) if v == below_world => self.bottom(),
            Some(v) => v,
            None => old,
        };
        if let Some(column) = data.sky_sources.get_mut(&(x >> 4, z >> 4)) {
            column[((x & 15) * 16 + (z & 15)) as usize] = new;
        }
        (old..new.max(old), new..old.max(new))
    }

    /// Whether light cannot come straight down from the block at `top` into the one under it.
    fn edge_occluded(&self, data: &LightData, x: i32, top: i32, z: i32) -> bool {
        let above = data.shapes.get(x, top, z).unwrap_or(0);
        let (filter, shape) = (data.props.get(x, top - 1, z).map_or(0, |v| v & 15), data.shapes.get(x, top - 1, z).unwrap_or(0));
        self.rules.edge_occluded(above, filter, shape)
    }
}
