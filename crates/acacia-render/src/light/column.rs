//! Lighting whole columns and single block changes against the world's current blocks.

use acacia_world::{BlockRegistry, Dimension, SECTION_VOLUME, World};

use super::data::{LightData, props};
use super::propagate::{Channel, Propagator, step};

const HORIZONTAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Light props byte per runtime id.
pub struct PropsTable(Vec<u8>);

impl PropsTable {
    pub fn new(registry: &BlockRegistry) -> Self {
        PropsTable((0..registry.len() as u32).map(|id| registry.get(id).map_or(15, |s| props(s.light_emission, s.light_filter))).collect())
    }

    /// Both layers: the brighter emission and the stronger filter (waterlogged blocks).
    #[inline]
    fn of(&self, block: u32, liquid: u32) -> u8 {
        let (a, b) = (self.get(block), self.get(liquid));
        (a & 0xF0).max(b & 0xF0) | (a & 15).max(b & 15)
    }

    #[inline]
    fn get(&self, id: u32) -> u8 {
        self.0.get(id as usize).copied().unwrap_or(15)
    }
}

pub struct Lighter<'a> {
    pub world: &'a World,
    pub table: &'a PropsTable,
    pub dim: Dimension,
}

impl Lighter<'_> {
    fn top(&self) -> i32 {
        self.dim.min_y + self.dim.height as i32
    }

    fn channels(&self) -> &'static [Channel] {
        if self.dim.sky { &[Channel::Block, Channel::Sky] } else { &[Channel::Block] }
    }

    fn section_range(&self) -> std::ops::Range<i32> {
        let min = self.dim.min_y >> 4;
        min..min + (self.dim.height / 16) as i32
    }

    /// Recomputes a column from its blocks: clears its old light (and what spread from it into
    /// loaded neighbours), then lights it and lets the neighbours' light back in. `false` when
    /// the column isn't loaded.
    pub fn relight_column(&self, data: &mut LightData, cx: i32, cz: i32) -> bool {
        let Some(shared) = self.world.get(cx, cz) else { return false };
        let was_loaded = !data.columns.insert((cx, cz));
        {
            let chunk = shared.read();
            let (mut blocks, mut liquid) = (Box::new([0; SECTION_VOLUME]), Box::new([0; SECTION_VOLUME]));
            let mut cells = Box::new([0u8; SECTION_VOLUME]);
            for sy in self.section_range() {
                let key = (cx, sy, cz);
                if chunk.copy_section((sy - (self.dim.min_y >> 4)) as usize, &mut blocks, &mut liquid) {
                    (0..SECTION_VOLUME).for_each(|i| cells[i] = self.table.of(blocks[i], liquid[i]));
                    data.props.insert(key, &cells);
                } else {
                    data.props.fill(key, self.table.of(self.dim.air, self.dim.air));
                }
                if !was_loaded {
                    data.light.fill(key, 0);
                }
            }
        }
        for &ch in self.channels() {
            if was_loaded {
                // Removal refills from the neighbours' remaining light, so it runs before seeding.
                let mut p = Propagator::new(data, ch, self.top());
                let mut lit = Vec::new();
                for sy in self.section_range() {
                    p.data.light.for_each_nonzero((cx, sy, cz), |i, v| {
                        if (v >> ch.shift()) & 15 > 0 {
                            lit.push(world_pos(cx, sy, cz, i));
                        }
                    });
                }
                lit.into_iter().for_each(|pos| p.remove(pos));
                p.run();
            }
            let mut p = Propagator::new(data, ch, self.top());
            match ch {
                Channel::Block => self.seed_emitters(&mut p, cx, cz),
                Channel::Sky => self.seed_sky(&mut p, cx, cz),
            }
            self.seed_from_neighbours(&mut p, cx, cz);
            p.run();
        }
        true
    }

    pub fn drop_column(&self, data: &mut LightData, cx: i32, cz: i32) {
        data.columns.remove(&(cx, cz));
        for sy in self.section_range() {
            data.light.remove((cx, sy, cz));
            data.props.remove((cx, sy, cz));
        }
    }

    /// Re-reads one block and updates the light around it.
    pub fn update_block(&self, data: &mut LightData, x: i32, y: i32, z: i32) {
        let Some(shared) = self.world.get(x >> 4, z >> 4) else { return };
        let new = {
            let chunk = shared.read();
            self.table.of(chunk.block(x, y, z), chunk.liquid(x, y, z))
        };
        if data.props.get(x, y, z).is_none_or(|old| old == new) {
            return;
        }
        data.props.set(x, y, z, new);
        for &ch in self.channels() {
            let mut p = Propagator::new(data, ch, self.top());
            p.remove([x, y, z]);
            p.run();
        }
    }

    fn seed_emitters(&self, p: &mut Propagator, cx: i32, cz: i32) {
        let mut emitters = Vec::new();
        for sy in self.section_range() {
            p.data.props.for_each_nonzero((cx, sy, cz), |i, v| {
                if v >> 4 > 0 {
                    emitters.push((world_pos(cx, sy, cz, i), v >> 4));
                }
            });
        }
        emitters.into_iter().for_each(|(pos, level)| p.raise(pos, level));
    }

    /// Fills the straight-down sky columns at 15, then spreads from wherever a neighbour may be
    /// darker: the end of each column and its sides that face a shorter column or the chunk edge.
    fn seed_sky(&self, p: &mut Propagator, cx: i32, cz: i32) {
        let (x0, z0, top) = (cx * 16, cz * 16, self.top());
        let mut lowest = [[top; 16]; 16];
        for lx in 0..16 {
            for lz in 0..16 {
                let (x, z) = (x0 + lx, z0 + lz);
                let mut level = 15;
                let mut y = top - 1;
                while y >= self.dim.min_y {
                    let filter = p.data.props.get(x, y, z).map_or(15, |v| v & 15);
                    level = step(level, filter, true);
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

    /// Lets light from loaded neighbour columns in through the shared faces.
    fn seed_from_neighbours(&self, p: &mut Propagator, cx: i32, cz: i32) {
        let (x0, z0) = (cx * 16, cz * 16);
        for i in 0..16 {
            for (x, z) in [(x0 - 1, z0 + i), (x0 + 16, z0 + i), (x0 + i, z0 - 1), (x0 + i, z0 + 16)] {
                for y in self.dim.min_y..self.top() {
                    if p.level([x, y, z]).is_some_and(|l| l > 1) {
                        p.spread_from([x, y, z]);
                    }
                }
            }
        }
    }
}

fn world_pos(cx: i32, sy: i32, cz: i32, i: usize) -> [i32; 3] {
    let i = i as i32;
    [cx * 16 + (i >> 8), sy * 16 + (i & 15), cz * 16 + ((i >> 4) & 15)]
}

#[cfg(test)]
#[test]
fn world_pos_inverts_index() {
    for p in [[0, 0, 0], [5, 9, 13], [15, 15, 15]] {
        assert_eq!(world_pos(-2, 3, 4, super::sections::index(p[0], p[1], p[2])), [-32 + p[0], 48 + p[1], 64 + p[2]]);
    }
}
