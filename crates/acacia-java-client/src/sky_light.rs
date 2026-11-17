//! The client's sky light, kept as "is it 15" per cell: `canSeeSky` is all the client asks of it. The server's data
//! (with each chunk and in `LightUpdate`) is queued and applied after the tick with the client's own relighting
//! (`relight.rs`), as `ClientLevel.update` runs both once a frame.

use std::collections::HashMap;

use acacia_java_proto::light::LightSection;

const FULL: u8 = 15;

#[derive(Debug, Clone)]
pub(crate) enum Layer {
    Dark,
    Full,
    /// One bit a cell in `y z x` order.
    Mixed(Box<[u64; 64]>),
}

impl Layer {
    fn of(section: LightSection) -> Layer {
        let LightSection::Data(nibbles) = section else { return Layer::Dark };
        let mut bits = Box::new([0u64; 64]);
        for (i, byte) in nibbles.iter().enumerate() {
            for (half, value) in [(0, byte & 15), (1, byte >> 4)] {
                if value == FULL {
                    let cell = i * 2 + half;
                    bits[cell / 64] |= 1 << (cell % 64);
                }
            }
        }
        match bits.iter().map(|w| w.count_ones()).sum::<u32>() {
            0 => Layer::Dark,
            4096 => Layer::Full,
            _ => Layer::Mixed(bits),
        }
    }

    pub(crate) fn full(&self, cell: usize) -> bool {
        match self {
            Layer::Dark => false,
            Layer::Full => true,
            Layer::Mixed(bits) => bits[cell / 64] >> (cell % 64) & 1 == 1,
        }
    }

    pub(crate) fn set(&mut self, cell: usize, full: bool) {
        if self.full(cell) == full {
            return;
        }
        if !matches!(self, Layer::Mixed(_)) {
            *self = Layer::Mixed(Box::new([if full { 0 } else { u64::MAX }; 64]));
        }
        if let Layer::Mixed(bits) = self {
            bits[cell / 64] ^= 1 << (cell % 64);
        }
    }

    /// `SkyLightSectionStorage.repeatFirstLayer`: the bottom plane copied up the section.
    pub(crate) fn repeat_bottom(&self) -> Layer {
        match self {
            Layer::Mixed(bits) => Layer::Mixed(Box::new(std::array::from_fn(|i| bits[i % 4]))),
            other => other.clone(),
        }
    }
}

/// The cell index of a position in its section.
pub(crate) fn cell([x, y, z]: [i32; 3]) -> usize {
    (((y & 15) << 8) | ((z & 15) << 4) | (x & 15)) as usize
}

/// Light sections per column, from the one below the lowest block section; `None` where none is stored.
#[derive(Debug, Clone, Default)]
pub(crate) struct SkyLight {
    columns: HashMap<(i32, i32), Vec<Option<Layer>>>,
    /// Server data (`None`: the chunk was forgotten) waiting for `ClientLevel.pollLightUpdates`.
    queued: Vec<((i32, i32), Option<Vec<(usize, LightSection)>>)>,
}

impl SkyLight {
    pub(crate) fn queue(&mut self, chunk: (i32, i32), sections: Vec<(usize, LightSection)>) {
        self.queued.push((chunk, Some(sections)));
    }

    pub(crate) fn queue_removal(&mut self, chunk: (i32, i32)) {
        self.queued.push((chunk, None));
    }

    /// `readSectionList`: the sections the packet names replace the old ones. A bot has one frame a tick, so the
    /// whole queue goes, where `pollLightUpdates` takes at least 10 a frame.
    pub(crate) fn apply_queued(&mut self) {
        for (chunk, sections) in std::mem::take(&mut self.queued) {
            let Some(sections) = sections else {
                self.columns.remove(&chunk);
                continue;
            };
            for (index, section) in sections {
                self.insert(chunk, index, Layer::of(section));
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.columns.clear();
        self.queued.clear();
    }

    /// `lightOnInSection`: the column has had light data.
    pub(crate) fn has_column(&self, chunk: (i32, i32)) -> bool {
        self.columns.contains_key(&chunk)
    }

    pub(crate) fn layer(&self, chunk: (i32, i32), index: usize) -> Option<&Layer> {
        self.columns.get(&chunk)?.get(index)?.as_ref()
    }

    pub(crate) fn layer_mut(&mut self, chunk: (i32, i32), index: usize) -> Option<&mut Layer> {
        self.columns.get_mut(&chunk)?.get_mut(index)?.as_mut()
    }

    /// One past the highest stored section (`topSections`); `isAboveData` from there up.
    pub(crate) fn top(&self, chunk: (i32, i32)) -> usize {
        self.columns.get(&chunk).and_then(|c| c.iter().rposition(Option::is_some)).map_or(0, |i| i + 1)
    }

    pub(crate) fn insert(&mut self, chunk: (i32, i32), index: usize, layer: Layer) {
        let column = self.columns.entry(chunk).or_default();
        if column.len() <= index {
            column.resize(index + 1, None);
        }
        column[index] = Some(layer);
    }

    pub(crate) fn remove(&mut self, chunk: (i32, i32), index: usize) {
        if let Some(slot) = self.columns.get_mut(&chunk).and_then(|c| c.get_mut(index)) {
            *slot = None;
        }
    }

    /// `canSeeSky` (sky light 15) as `SkyLightSectionStorage.getLightValue` reads it: a section without data takes
    /// the bottom layer of the next one up that has some, and none above (or no column at all) is 15.
    pub(crate) fn sees_sky(&self, [x, y, z]: [i32; 3], min_y: i32) -> bool {
        let Some(column) = self.columns.get(&(x >> 4, z >> 4)) else { return true };
        let mut index = (y >> 4) - (min_y >> 4) + 1;
        let mut local_y = y & 15;
        loop {
            match usize::try_from(index).ok().map(|i| column.get(i)) {
                Some(None) => return true,
                Some(Some(Some(layer))) => return layer.full(cell([x, local_y, z])),
                _ => {
                    index += 1;
                    local_y = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(fill: impl Fn(usize) -> u8) -> LightSection {
        LightSection::Data((0..2048).map(|i| fill(i * 2) | fill(i * 2 + 1) << 4).collect())
    }

    #[test]
    fn a_roof_shades_the_cells_under_it_and_gaps_read_the_layer_above() {
        let mut sky = SkyLight::default();
        // At min y 0, light section 2 is y 16..31: lit above y 20, and only at x 0.
        let lit = layer(|cell| if cell >> 8 > 4 && cell & 15 == 0 { 15 } else { 0 });
        sky.queue((0, 0), vec![(0, LightSection::Empty), (2, lit), (4, LightSection::Empty)]);
        assert!(sky.sees_sky([1, 30, 0], 0), "queued data waits for the frame");
        sky.apply_queued();
        assert!(sky.sees_sky([0, 30, 0], 0) && !sky.sees_sky([1, 30, 0], 0) && !sky.sees_sky([0, 20, 0], 0));
        assert!(sky.sees_sky([0, 200, 0], 0), "above every section");
        assert!(!sky.sees_sky([0, 40, 0], 0), "no data in section 3: the one above is dark");
        assert!(!sky.sees_sky([0, -10, 0], 0), "an empty section is dark");
        assert!(sky.sees_sky([16, 0, 0], 0), "unloaded");
    }

    #[test]
    fn cells_flip_and_the_bottom_plane_repeats() {
        let mut layer = Layer::Full;
        layer.set(cell([3, 0, 2]), false);
        assert!(!layer.full(cell([3, 0, 2])) && layer.full(cell([4, 0, 2])));
        let repeated = layer.repeat_bottom();
        assert!(!repeated.full(cell([3, 9, 2])) && repeated.full(cell([3, 9, 3])));
    }
}
