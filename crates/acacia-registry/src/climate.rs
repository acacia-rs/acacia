//! Java biome climates and what falls from the sky: 26.3's `Biome.getPrecipitationAt` from `ClimateSettings`
//! (temperature, `temperature_modifier`, `has_precipitation`).

use std::sync::LazyLock;

use crate::simplex::{Lcg, Simplex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    pub temperature: f32,
    /// `temperature_modifier: frozen` (frozen oceans): patches of ground as warm as 0.2.
    pub frozen: bool,
    pub has_precipitation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precipitation {
    None,
    Rain,
    Snow,
}

/// 26.3's vanilla biomes by path: temperature, `has_precipitation`, frozen.
#[rustfmt::skip]
const VANILLA: &[(&str, f32, bool, bool)] = &[
    ("badlands", 2.0, false, false), ("bamboo_jungle", 0.95, true, false), ("basalt_deltas", 2.0, false, false),
    ("beach", 0.8, true, false), ("birch_forest", 0.6, true, false), ("cherry_grove", 0.5, true, false),
    ("cold_ocean", 0.5, true, false), ("crimson_forest", 2.0, false, false), ("dappled_forest", 0.6, true, false),
    ("dark_forest", 0.7, true, false), ("deep_cold_ocean", 0.5, true, false), ("deep_dark", 0.8, true, false),
    ("deep_frozen_ocean", 0.5, true, true), ("deep_lukewarm_ocean", 0.5, true, false),
    ("deep_ocean", 0.5, true, false), ("desert", 2.0, false, false), ("dripstone_caves", 0.8, true, false),
    ("end_barrens", 0.5, false, false), ("end_highlands", 0.5, false, false), ("end_midlands", 0.5, false, false),
    ("eroded_badlands", 2.0, false, false), ("flower_forest", 0.7, true, false), ("forest", 0.7, true, false),
    ("frozen_ocean", 0.0, true, true), ("frozen_peaks", -0.7, true, false), ("frozen_river", 0.0, true, false),
    ("grove", -0.2, true, false), ("ice_spikes", 0.0, true, false), ("jagged_peaks", -0.7, true, false),
    ("jungle", 0.95, true, false), ("lukewarm_ocean", 0.5, true, false), ("lush_caves", 0.5, true, false),
    ("mangrove_swamp", 0.8, true, false), ("meadow", 0.5, true, false), ("mushroom_fields", 0.9, true, false),
    ("nether_wastes", 2.0, false, false), ("ocean", 0.5, true, false), ("old_growth_birch_forest", 0.6, true, false),
    ("old_growth_pine_taiga", 0.3, true, false), ("old_growth_spruce_taiga", 0.25, true, false),
    ("pale_garden", 0.7, true, false), ("plains", 0.8, true, false), ("river", 0.5, true, false),
    ("savanna", 2.0, false, false), ("savanna_plateau", 2.0, false, false), ("small_end_islands", 0.5, false, false),
    ("snowy_beach", 0.05, true, false), ("snowy_plains", 0.0, true, false), ("snowy_slopes", -0.3, true, false),
    ("snowy_taiga", -0.5, true, false), ("soul_sand_valley", 2.0, false, false),
    ("sparse_jungle", 0.95, true, false), ("stony_peaks", 1.0, true, false), ("stony_shore", 0.2, true, false),
    ("sulfur_caves", 0.8, true, false), ("sunflower_plains", 0.8, true, false), ("swamp", 0.8, true, false),
    ("taiga", 0.25, true, false), ("the_end", 0.5, false, false), ("the_void", 0.5, false, false),
    ("warm_ocean", 0.5, true, false), ("warped_forest", 2.0, false, false), ("windswept_forest", 0.2, true, false),
    ("windswept_gravelly_hills", 0.2, true, false), ("windswept_hills", 0.2, true, false),
    ("windswept_savanna", 2.0, false, false), ("wooded_badlands", 2.0, false, false),
];

static TEMPERATURE: LazyLock<Simplex> = LazyLock::new(|| Simplex::draw(&mut Lcg::new(1234)));
static BIOME_INFO: LazyLock<Simplex> = LazyLock::new(|| Simplex::draw(&mut Lcg::new(2345)));
/// `FROZEN_TEMPERATURE_NOISE`: three noises from one random, at halving frequencies.
static FROZEN: LazyLock<[Simplex; 3]> = LazyLock::new(|| {
    let mut random = Lcg::new(3456);
    std::array::from_fn(|_| Simplex::draw(&mut random))
});
const FROZEN_LAYERS: [(f64, f32); 3] = [(1.0, 0.142_857_15), (0.5, 0.285_714_3), (0.25, 0.571_428_6)];

impl Climate {
    /// A vanilla biome's climate by id (`minecraft:plains`) or path.
    pub fn vanilla(name: &str) -> Option<Climate> {
        let path = name.strip_prefix("minecraft:").unwrap_or(name);
        VANILLA.iter().find(|(n, ..)| *n == path).map(|&(_, temperature, has_precipitation, frozen)| Climate { temperature, frozen, has_precipitation })
    }

    pub fn precipitation_at(&self, pos: [i32; 3], sea_level: i32) -> Precipitation {
        match () {
            _ if !self.has_precipitation => Precipitation::None,
            _ if self.temperature_at(pos, sea_level) < 0.15 => Precipitation::Snow,
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

/// `TemperatureModifier.FROZEN`: 0.2 in its ice-free patches.
fn frozen_ground([x, z]: [i32; 2]) -> Option<f32> {
    let (x, z) = (f64::from(x), f64::from(z));
    let large: f32 = FROZEN.iter().zip(FROZEN_LAYERS).fold(0.0, |sum, (noise, (frequency, amplitude))| sum + amplitude * noise.get(x * 0.05 * frequency, z * 0.05 * frequency));
    let patches = f64::from(large * 7.0) + f64::from(BIOME_INFO.get(x * 0.2, z * 0.2));
    (patches < 0.3 && f64::from(BIOME_INFO.get(x * 0.09, z * 0.09)) < 0.8).then_some(0.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEA_LEVEL: i32 = 63;

    #[test]
    fn plains_rain_deserts_stay_dry_and_snowy_plains_snow() {
        let at = |name: &str| Climate::vanilla(name).unwrap().precipitation_at([0, 64, 0], SEA_LEVEL);
        assert_eq!((at("minecraft:plains"), at("desert"), at("snowy_plains")), (Precipitation::Rain, Precipitation::None, Precipitation::Snow));
    }

    #[test]
    fn hills_snow_on_their_peaks() {
        let hills = Climate::vanilla("windswept_hills").unwrap();
        assert_eq!((hills.precipitation_at([0, 70, 0], SEA_LEVEL), hills.precipitation_at([0, 160, 0], SEA_LEVEL)), (Precipitation::Rain, Precipitation::Snow));
        assert_eq!(hills.temperature_at([5, 80, 9], SEA_LEVEL), 0.2, "at sea level + 17");
    }

    #[test]
    fn frozen_oceans_have_warmer_patches() {
        let ocean = Climate::vanilla("frozen_ocean").unwrap();
        let warm = (0..400).filter(|i| ocean.temperature_at([i * 13, 63, i * -7], SEA_LEVEL) == 0.2).count();
        assert!((1..400).contains(&warm), "{warm} of 400 columns in an ice-free patch");
    }
}
