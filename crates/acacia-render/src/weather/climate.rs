//! What falls from the sky at a position: 26.3's `Biome.getPrecipitationAt`, from the biome's
//! `ClimateSettings` (temperature, `temperature_modifier`, `has_precipitation`).

use std::sync::LazyLock;

use crate::biome::noise::{BIOME_INFO, Simplex};
use crate::blocks::placed::Draw;

/// The overworld's `getSeaLevel`; rain falls nowhere else.
pub const SEA_LEVEL: i32 = 63;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    pub temperature: f32,
    /// `temperature_modifier: frozen` (frozen ocean): patches of ground as cold as 0.2.
    pub frozen: bool,
    pub has_precipitation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precipitation {
    None,
    Rain,
    Snow,
}

static TEMPERATURE: LazyLock<Simplex> = LazyLock::new(|| Simplex::from_draw(&mut Draw::new(1234)));

/// `FROZEN_TEMPERATURE_NOISE`: three noises from one random, at halving frequencies.
static FROZEN: LazyLock<[Simplex; 3]> = LazyLock::new(|| {
    let mut draw = Draw::new(3456);
    std::array::from_fn(|_| Simplex::from_draw(&mut draw))
});
const FROZEN_LAYERS: [(f64, f32); 3] = [(1.0, 0.142_857_15), (0.5, 0.285_714_3), (0.25, 0.571_428_6)];

impl Climate {
    pub fn precipitation_at(&self, [x, y, z]: [i32; 3], sea_level: i32) -> Precipitation {
        match () {
            _ if !self.has_precipitation => Precipitation::None,
            _ if self.temperature_at([x, y, z], sea_level) < 0.15 => Precipitation::Snow,
            _ => Precipitation::Rain,
        }
    }

    /// `getHeightAdjustedTemperature`: colder above `sea_level + 17`, by a noisy amount.
    pub fn temperature_at(&self, [x, y, z]: [i32; 3], sea_level: i32) -> f32 {
        let base = if self.frozen { frozen_ground([x, z]).unwrap_or(self.temperature) } else { self.temperature };
        let snow_level = sea_level + 17;
        if y <= snow_level {
            return base;
        }
        let v = TEMPERATURE.get(f64::from(x as f32 / 8.0), f64::from(z as f32 / 8.0)) * 8.0;
        base - (v + y as f32 - snow_level as f32) * 0.05 / 40.0
    }
}

/// `TemperatureModifier.FROZEN`: 0.2 in its ice patches.
fn frozen_ground([x, z]: [i32; 2]) -> Option<f32> {
    let (x, z) = (f64::from(x), f64::from(z));
    let large: f32 = FROZEN.iter().zip(FROZEN_LAYERS).fold(0.0, |sum, (noise, (frequency, amplitude))| sum + amplitude * noise.get(x * 0.05 * frequency, z * 0.05 * frequency));
    let patches = f64::from(large * 7.0) + f64::from(BIOME_INFO.get(x * 0.2, z * 0.2));
    (patches < 0.3 && f64::from(BIOME_INFO.get(x * 0.09, z * 0.09)) < 0.8).then_some(0.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn climate(temperature: f32, has_precipitation: bool) -> Climate {
        Climate { temperature, frozen: false, has_precipitation }
    }

    #[test]
    fn dry_biomes_get_nothing_and_cold_ones_snow() {
        let at = |c: Climate, y| c.precipitation_at([0, y, 0], SEA_LEVEL);
        assert_eq!((at(climate(0.8, true), 64), at(climate(2.0, false), 64), at(climate(0.0, true), 64)), (Precipitation::Rain, Precipitation::None, Precipitation::Snow));
    }

    #[test]
    fn it_cools_above_the_snow_level_by_a_noisy_amount() {
        let plains = climate(0.8, true);
        assert_eq!(plains.temperature_at([5, 80, 9], SEA_LEVEL), 0.8, "at sea level + 17");
        // 0.05 per 40 blocks, the noise shifting the height up to 16 blocks either way.
        let cooled = plains.temperature_at([5, 400, 9], SEA_LEVEL);
        let expected = 0.8 - 320.0 * 0.05 / 40.0;
        assert!((cooled - expected).abs() <= 16.0 * 0.05 / 40.0 + 1e-6 && cooled != expected, "{cooled}");
        // Windswept hills (0.2) snow on their peaks.
        let hills = climate(0.2, true);
        assert_eq!((hills.precipitation_at([0, 70, 0], SEA_LEVEL), hills.precipitation_at([0, 160, 0], SEA_LEVEL)), (Precipitation::Rain, Precipitation::Snow));
    }

    #[test]
    fn frozen_oceans_have_warmer_and_colder_ground() {
        let ocean = Climate { temperature: 0.0, frozen: true, has_precipitation: true };
        let warm = (0..400).filter(|i| ocean.temperature_at([i * 13, 63, i * -7], SEA_LEVEL) == 0.2).count();
        assert!((1..400).contains(&warm), "{warm} of 400 columns in an ice-free patch");
    }
}
