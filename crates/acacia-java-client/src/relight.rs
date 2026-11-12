//! The client's sky relighting after block changes (`LevelChunk.setBlockState` → `checkBlock`, run by
//! `ClientLevel.update` after the tick), as far as sky light 15 goes: that is all `canSeeSky` reads. Each step
//! sideways or down costs at least 1 (`LightEngine.getOpacity`), so a 15 only comes from `SkyLightEngine`'s column
//! sources: `checkNode` → `updateSourcesInColumn` sets a column's 15s from `ChunkSkyLightSources`' lowest source,
//! and the propagation after it never makes or clears one. Boundary: levels below 15 aren't kept; the sources are
//! rescanned per check instead of kept per chunk; section storage counts neighbours only in loaded chunks; queued
//! server data is applied whole each frame (`pollLightUpdates` may split it).

use std::collections::HashSet;

use crate::chunks::is_air;
use crate::sky_light::{Layer, cell};
use crate::sky_props::{edge_occluded, light_properties_differ};
use crate::world::World;

/// Light changes waiting for the frame: `blockNodesToCheck`, and sections whose neighbourhood emptied.
#[derive(Debug, Default)]
pub(crate) struct Pending {
    checks: Vec<[i32; 3]>,
    emptied: HashSet<[i32; 3]>,
}

fn section_of([x, y, z]: [i32; 3]) -> [i32; 3] {
    [x >> 4, y >> 4, z >> 4]
}

fn around([sx, sy, sz]: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    (-1..=1).flat_map(move |dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [sx + dx, sy + dy, sz + dz])))
}

impl World {
    /// The light side of a block change from `old` to `new`: section storage as `updateSectionStatus` keeps it
    /// (at once), and a `checkBlock` for the frame.
    pub(crate) fn note_light_change(&mut self, pos: [i32; 3], old: u32, new: u32) {
        let section = section_of(pos);
        if is_air(old) != is_air(new) && self.chunks.only_air(section) == Some(is_air(new)) {
            if is_air(new) {
                self.light.emptied.extend(around(section));
            } else {
                around(section).for_each(|s| self.store_section(s));
            }
        }
        if light_properties_differ(old, new) {
            self.light.checks.push(pos);
        }
    }

    /// `ClientLevel.update`: the checks against the stored light, then the server's queued data and dropped
    /// sections, as `runLightUpdates` ends with `markNewInconsistencies`.
    pub(crate) fn update_light(&mut self) {
        for pos in std::mem::take(&mut self.light.checks) {
            self.check_sky(pos);
        }
        for section in std::mem::take(&mut self.light.emptied) {
            if !around(section).any(|s| self.chunks.only_air(s) == Some(false)) {
                self.sky.remove((section[0], section[2]), self.light_index(section[1]));
            }
        }
        self.sky.apply_queued();
    }

    /// Light section index of section `sy`: they start one below the lowest block section.
    fn light_index(&self, sy: i32) -> usize {
        usize::try_from(sy - (self.min_y() >> 4) + 1).unwrap_or(0)
    }

    /// `initializeSection` with `SkyLightSectionStorage.createDataLayer`: under stored data, the bottom plane of the
    /// next stored section up; above it, full.
    fn store_section(&mut self, [sx, sy, sz]: [i32; 3]) {
        let chunk = (sx, sz);
        let (index, light_sections) = (self.light_index(sy), self.chunks.max_y(sx * 16, sz * 16).map(|top| self.light_index(top >> 4) + 1));
        if !self.sky.has_column(chunk) || sy < (self.min_y() >> 4) - 1 || light_sections.is_some_and(|n| index >= n) || self.sky.layer(chunk, index).is_some() {
            return;
        }
        let above = (index + 1..self.sky.top(chunk)).find_map(|i| self.sky.layer(chunk, i));
        let layer = above.map_or(Layer::Full, Layer::repeat_bottom);
        self.sky.insert(chunk, index, layer);
    }

    /// `ChunkSkyLightSources`' lowest source of a column, scanned from the top; `i32::MIN` when the sky reaches
    /// below the world. `update` keeps the same value incrementally.
    fn lowest_source(&self, x: i32, z: i32) -> Option<i32> {
        let top = self.chunks.max_y(x, z)?;
        let mut above = 0;
        for y in (self.min_y()..top).rev() {
            let below = self.block([x, y, z]).unwrap_or(0);
            if edge_occluded(above, below) {
                return Some(y + 1);
            }
            above = below;
        }
        Some(i32::MIN)
    }

    /// `SkyLightEngine.checkNode` for the 15s.
    fn check_sky(&mut self, pos @ [x, y, z]: [i32; 3]) {
        let chunk = (x >> 4, z >> 4);
        if !self.sky.has_column(chunk) {
            return;
        }
        let Some(lowest) = self.lowest_source(x, z) else { return };
        let bottom = (self.min_y() >> 4) - 1;
        let bottom_y = bottom * 16;
        let index_of = |y: i32| usize::try_from((y >> 4) - bottom).ok();
        // `removeSourcesBelow`: down from the source, until a cell isn't 15.
        if lowest > bottom_y {
            let start = lowest - 1;
            'remove: for index in (0..=index_of(start).unwrap_or(0)).rev() {
                let Some(layer) = self.sky.layer_mut(chunk, index) else { continue };
                let section_bottom = (bottom + index as i32) * 16;
                for y in (section_bottom..=start.min(section_bottom + 15)).rev() {
                    if !layer.full(cell([x, y, z])) {
                        break 'remove;
                    }
                    layer.set(cell([x, y, z]), false);
                }
            }
        }
        // `addSourcesAbove`: up from the source, until a cell already is.
        let start = lowest.max(bottom_y);
        'add: for index in index_of(start).unwrap_or(0)..self.sky.top(chunk) {
            let Some(layer) = self.sky.layer_mut(chunk, index) else { continue };
            let section_bottom = (bottom + index as i32) * 16;
            for y in start.max(section_bottom)..=section_bottom + 15 {
                if layer.full(cell([x, y, z])) {
                    break 'add;
                }
                layer.set(cell([x, y, z]), true);
            }
        }
        // A checked cell below the sources loses its light.
        if y < lowest
            && let Some(layer) = index_of(y).and_then(|i| self.sky.layer_mut(chunk, i))
        {
            layer.set(cell(pos), false);
        }
    }
}

#[cfg(test)]
#[path = "relight_tests.rs"]
mod tests;
