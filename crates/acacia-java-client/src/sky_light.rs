//! The sky light the server sends (with each chunk and in `LightUpdate`), kept as "is it 15" per cell: `canSeeSky`
//! is all the client asks of it. The vanilla client also relights after block changes; this waits for the server's
//! update instead.

use std::collections::HashMap;

use acacia_java_proto::light::LightSection;

const FULL: u8 = 15;

#[derive(Debug, Clone)]
enum Layer {
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

    fn full(&self, cell: usize) -> bool {
        match self {
            Layer::Dark => false,
            Layer::Full => true,
            Layer::Mixed(bits) => bits[cell / 64] >> (cell % 64) & 1 == 1,
        }
    }
}

/// Light sections per column, from the one below the lowest block section; `None` where nothing was sent.
#[derive(Debug, Clone, Default)]
pub(crate) struct SkyLight {
    columns: HashMap<(i32, i32), Vec<Option<Layer>>>,
}

impl SkyLight {
    /// `readSectionList`: the sections the packet names replace the old ones.
    pub(crate) fn set(&mut self, chunk: (i32, i32), sections: Vec<(usize, LightSection)>) {
        let column = self.columns.entry(chunk).or_default();
        for (index, section) in sections {
            if column.len() <= index {
                column.resize(index + 1, None);
            }
            column[index] = Some(Layer::of(section));
        }
    }

    pub(crate) fn remove(&mut self, chunk: (i32, i32)) {
        self.columns.remove(&chunk);
    }

    pub(crate) fn clear(&mut self) {
        self.columns.clear();
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
                Some(Some(Some(layer))) => return layer.full(((local_y << 8) | ((z & 15) << 4) | (x & 15)) as usize),
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
        sky.set((0, 0), vec![(0, LightSection::Empty), (2, lit), (4, LightSection::Empty)]);
        assert!(sky.sees_sky([0, 30, 0], 0) && !sky.sees_sky([1, 30, 0], 0) && !sky.sees_sky([0, 20, 0], 0));
        assert!(sky.sees_sky([0, 200, 0], 0), "above every section");
        assert!(!sky.sees_sky([0, 40, 0], 0), "no data in section 3: the one above is dark");
        assert!(!sky.sees_sky([0, -10, 0], 0), "an empty section is dark");
        assert!(sky.sees_sky([16, 0, 0], 0), "unloaded");
    }
}
