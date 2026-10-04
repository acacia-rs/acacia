//! Day and night from the time of day, with Java's formulas (Bedrock's look the same).

use std::f32::consts::{PI, TAU};

use image::RgbaImage;

use std::path::Path;

use crate::assets::image_file;

/// Ticks in a day; 0 is sunrise, 6000 noon, 18000 midnight.
pub const DAY_TICKS: f32 = 24000.0;
pub const NOON: f32 = 6000.0;
/// Daytime sky and fog colour (sRGB).
const DAY: [f32; 3] = [0.62, 0.76, 1.0];
/// Sky light levels lost at midnight.
const NIGHT_DARKEN: f32 = 11.0;
const MOON_PHASES: i64 = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sky {
    /// Sky and fog colour (sRGB).
    pub color: [f32; 3],
    /// Levels taken off every cell's sky light.
    pub darken: f32,
    /// Turns the sun has made past noon, 0..1. It lingers near noon and midnight.
    pub turn: f32,
    /// Brightness of the stars, up to 0.5.
    pub stars: f32,
}

impl Sky {
    pub fn at(time: f32) -> Sky {
        let day = (time / DAY_TICKS - 0.25).rem_euclid(1.0);
        let turn = (day * 2.0 + 0.5 - (day * PI).cos() / 2.0) / 3.0;
        // Height of the sun: 1 at noon, -1 at midnight.
        let height = (turn * TAU).cos();
        let light = (height * 2.0 + 0.5).clamp(0.0, 1.0);
        let [r, g, b] = DAY;
        let stars = (0.75 - height * 2.0).clamp(0.0, 1.0);
        Sky {
            color: [r * (light * 0.94 + 0.06), g * (light * 0.94 + 0.06), b * (light * 0.91 + 0.09)],
            darken: (1.0 - (height * 2.0 + 0.2).clamp(0.0, 1.0)) * NIGHT_DARKEN,
            turn,
            stars: stars * stars * 0.5,
        }
    }
}

/// The moon's phase on the day a world time in ticks falls on: 0 is full, 4 new.
pub fn moon_phase(world_time: i64) -> u8 {
    world_time.div_euclid(DAY_TICKS as i64).rem_euclid(MOON_PHASES) as u8
}

/// `textures/environment/sun.png` and `moon_phases.png` (4×2 phases).
pub struct SkyTextures {
    pub sun: RgbaImage,
    pub moon: RgbaImage,
}

impl SkyTextures {
    pub fn load(root: &Path) -> Option<SkyTextures> {
        let open = |name: &str| Some(image::open(image_file(root, &format!("textures/environment/{name}"))?).ok()?.into_rgba8());
        Some(SkyTextures { sun: open("sun")?, moon: open("moon_phases")? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_bright_and_midnight_dark() {
        let noon = Sky::at(NOON);
        assert_eq!((noon.color, noon.darken, noon.turn, noon.stars), (DAY, 0.0, 0.0, 0.0));
        let midnight = Sky::at(18000.0);
        assert_eq!((midnight.darken, midnight.stars), (NIGHT_DARKEN, 0.5));
        assert!(midnight.color[2] < 0.1 && (midnight.turn - 0.5).abs() < 1e-6, "{midnight:?}");
        // Dusk falls between sunset (12000) and 13670, when the last sky light is gone.
        let dusk = Sky::at(13000.0);
        assert!(dusk.darken > 3.0 && dusk.darken < NIGHT_DARKEN, "{dusk:?}");
        assert_eq!(Sky::at(NOON + DAY_TICKS * 3.0), noon, "the time wraps");
        assert_eq!((moon_phase(23999), moon_phase(24000), moon_phase(24000 * 9), moon_phase(-1)), (0, 1, 1, 7));
    }
}
