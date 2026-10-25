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

    /// Java's rules: the lowest source of every block column of a column.
    pub(super) fn find_sky_sources(&self, data: &mut LightData, cx: i32, cz: i32) {
        let mut lowest = Box::new([0; 256]);
        for (i, low) in lowest.iter_mut().enumerate() {
            *low = self.lowest_source(data, cx * 16 + (i as i32 >> 4), cz * 16 + (i as i32 & 15));
        }
        data.sky_sources.insert((cx, cz), lowest);
    }

    /// Java's rules, after a block of the column at `x`, `z` changed: the heights that stopped
    /// being sources and those that became sources.
    pub(super) fn move_sky_sources(&self, data: &mut LightData, x: i32, z: i32) -> (Range<i32>, Range<i32>) {
        let Some(old) = data.sky_lowest(x, z) else { return (0..0, 0..0) };
        let new = self.lowest_source(data, x, z);
        if let Some(column) = data.sky_sources.get_mut(&(x >> 4, z >> 4)) {
            column[((x & 15) * 16 + (z & 15)) as usize] = new;
        }
        (old..new.max(old), new..old.max(new))
    }

    fn lowest_source(&self, data: &LightData, x: i32, z: i32) -> i32 {
        let mut above = 0;
        for y in (self.bottom()..self.top()).rev() {
            let (filter, shape) = (data.props.get(x, y, z).map_or(15, |v| v & 15), data.shapes.get(x, y, z).unwrap_or(0));
            if self.rules.edge_occluded(above, filter, shape) {
                return y + 1;
            }
            above = shape;
        }
        self.bottom()
    }
}
