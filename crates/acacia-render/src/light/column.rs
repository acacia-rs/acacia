//! Lighting whole columns and single block changes against the world's current blocks.

use acacia_world::{Dimension, World};

use super::blocks::{Cell, LightBlocks};
use super::data::LightData;
use super::propagate::{Channel, Propagator};
use super::rules::Rules;

/// The engine over some blocks: `dim.sky` says whether it lights the sky channel.
pub struct Lighter<'a, B> {
    pub blocks: B,
    pub rules: Rules<'a>,
    pub dim: Dimension,
}

/// A [`Lighter`] over a [`World`].
pub type WorldLighter<'a> = Lighter<'a, super::blocks::WorldBlocks<'a>>;

impl<'a, B: LightBlocks> Lighter<'a, B> {
    /// Java keeps light for a section of air below and above the world (`LIGHT_SECTION_PADDING`),
    /// and light goes round through them.
    fn padding(&self) -> i32 {
        if self.rules.is_java() { 1 } else { 0 }
    }

    pub(super) fn top(&self) -> i32 {
        self.dim.min_y + self.dim.height as i32 + 16 * self.padding()
    }

    pub(super) fn bottom(&self) -> i32 {
        self.dim.min_y - 16 * self.padding()
    }

    fn channels(&self) -> &'static [Channel] {
        if self.dim.sky { &[Channel::Block, Channel::Sky] } else { &[Channel::Block] }
    }

    /// The world's sections and the padding.
    pub(super) fn section_range(&self) -> std::ops::Range<i32> {
        let min = self.dim.min_y >> 4;
        min - self.padding()..min + (self.dim.height / 16) as i32 + self.padding()
    }

    pub(super) fn propagator<'d>(&self, data: &'d mut LightData, ch: Channel) -> Propagator<'d>
    where
        'a: 'd,
    {
        Propagator::new(data, ch, self.top(), self.rules)
    }

    /// Recomputes a column from its blocks: clears its old light (and what spread from it into
    /// loaded neighbours), then lights it and lets the neighbours' light back in. `false` when
    /// the column isn't loaded.
    pub fn relight_column(&self, data: &mut LightData, cx: i32, cz: i32) -> bool {
        let Some(sections) = self.blocks.column(cx, cz) else { return false };
        let was_loaded = !data.columns.insert((cx, cz));
        let air = self.blocks.air();
        let padding = std::iter::repeat_with(|| None).take(self.padding() as usize);
        let sections = padding.clone().chain(sections).chain(padding);
        let mut empty = Vec::new();
        for (sy, cells) in self.section_range().zip(sections) {
            let key = (cx, sy, cz);
            empty.push(cells.is_none());
            match cells {
                Some(cells) => {
                    data.props.insert(key, &cells.map(|c| c.props));
                    data.shapes.insert(key, &cells.map(|c| c.shape));
                }
                None => {
                    data.props.fill(key, air.props);
                    data.shapes.fill(key, air.shape);
                }
            }
            if !was_loaded {
                data.light.fill(key, 0);
            }
        }
        for &ch in self.channels() {
            if was_loaded {
                // Removal refills from the neighbours' remaining light, so it runs before seeding.
                let mut p = self.propagator(data, ch);
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
            if ch == Channel::Sky && self.rules.is_java() {
                self.find_sky_sources(data, cx, cz, &empty);
            }
            let mut p = self.propagator(data, ch);
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
        data.sky_sources.remove(&(cx, cz));
        for sy in self.section_range() {
            data.light.remove((cx, sy, cz));
            data.props.remove((cx, sy, cz));
            data.shapes.remove((cx, sy, cz));
        }
    }

    /// Re-reads one block and updates the light around it.
    pub fn update_block(&self, data: &mut LightData, x: i32, y: i32, z: i32) {
        let Some(new) = self.blocks.cell(x, y, z) else { return };
        let (Some(props), Some(shape)) = (data.props.get(x, y, z), data.shapes.get(x, y, z)) else { return };
        if (Cell { props, shape }) == new {
            return;
        }
        data.props.set(x, y, z, new.props);
        data.shapes.set(x, y, z, new.shape);
        for &ch in self.channels() {
            let sources = (ch == Channel::Sky && self.rules.is_java()).then(|| self.move_sky_sources(data, x, y, z));
            let mut p = self.propagator(data, ch);
            p.remove([x, y, z]);
            if let Some((lost, gained)) = sources {
                lost.for_each(|y| p.remove([x, y, z]));
                gained.for_each(|y| p.raise([x, y, z], 15));
            }
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

    /// Lets light from loaded neighbour columns in through the shared faces.
    fn seed_from_neighbours(&self, p: &mut Propagator, cx: i32, cz: i32) {
        let (x0, z0) = (cx * 16, cz * 16);
        for i in 0..16 {
            for (x, z) in [(x0 - 1, z0 + i), (x0 + 16, z0 + i), (x0 + i, z0 - 1), (x0 + i, z0 + 16)] {
                for y in self.bottom()..self.top() {
                    if p.level([x, y, z]).is_some_and(|l| l > 1) {
                        p.spread_from([x, y, z]);
                    }
                }
            }
        }
    }
}

impl<'a> WorldLighter<'a> {
    pub fn new(world: &'a World, table: &'a super::blocks::PropsTable, dim: Dimension) -> Self {
        Lighter { blocks: super::blocks::WorldBlocks { world, table }, rules: table.rules(), dim }
    }
}

pub(super) fn world_pos(cx: i32, sy: i32, cz: i32, i: usize) -> [i32; 3] {
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
