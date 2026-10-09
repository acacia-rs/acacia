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
    /// How visible the sun and moon are, 0 to 1.
    pub celestial: f32,
    /// The sunrise or sunset glow (sRGB and its strength) while the sun is near the horizon, and
    /// the side it is on: +1 east (+x), -1 west.
    pub glow: Option<([f32; 4], f32)>,
}

/// Weather on the sky, from Java's `ClientLevel.getSkyColor` and `Level.updateSkyBrightness`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Weather {
    /// 0 to 1.
    pub rain: f32,
    pub thunder: f32,
    /// A lightning flash, 0 to 1 (Java's sky flash eased over its last tick).
    pub flash: f32,
}

impl Sky {
    pub fn at(time: f32, weather: Weather) -> Sky {
        let day = (time / DAY_TICKS - 0.25).rem_euclid(1.0);
        let turn = (day * 2.0 + 0.5 - (day * PI).cos() / 2.0) / 3.0;
        // Height of the sun: 1 at noon, -1 at midnight.
        let height = (turn * TAU).cos();
        let light = (height * 2.0 + 0.5).clamp(0.0, 1.0);
        let [r, g, b] = DAY;
        let clear = 1.0 - weather.rain;
        let stars = (0.75 - height * 2.0).clamp(0.0, 1.0);
        let daylight = (height * 2.0 + 0.2).clamp(0.0, 1.0) * (1.0 - weather.rain * 5.0 / 16.0) * (1.0 - weather.thunder * 5.0 / 16.0);
        let color = [r * (light * 0.94 + 0.06), g * (light * 0.94 + 0.06), b * (light * 0.91 + 0.09)];
        // The sun sets in the west through the first half of its turn and rises in the east in the second.
        let glow = sunrise(height).map(|c| (c, if turn < 0.5 { -1.0 } else { 1.0 }));
        Sky { color: weathered(color, weather), darken: (1.0 - daylight) * NIGHT_DARKEN, turn, stars: stars * stars * 0.5 * clear, celestial: clear, glow }
    }

    /// The sky and fog colour seen looking along `forward`: pulled to the glow when facing it
    /// (Java's `FogRenderer`).
    pub fn color_towards(&self, forward: glam::Vec3) -> [f32; 3] {
        let Some(([r, g, b, strength], side)) = self.glow else { return self.color };
        let facing = (forward.x * side).max(0.0) * strength;
        let glow = [r, g, b];
        std::array::from_fn(|i| self.color[i] * (1.0 - facing) + glow[i] * facing)
    }
}

/// Java's `getSunriseOrSunsetColor`: `height` is the sun's (1 at noon, 0 on the horizon).
fn sunrise(height: f32) -> Option<[f32; 4]> {
    if !(-0.4..=0.4).contains(&height) {
        return None;
    }
    let h = height / 0.4 * 0.5 + 0.5;
    let strength = (1.0 - (1.0 - (h * PI).sin()) * 0.99).powi(2);
    Some([h * 0.3 + 0.7, h * h * 0.7 + 0.2, 0.2, strength])
}

/// White clouds greyed by the weather (`ClientLevel.getCloudColor`), linear.
pub fn cloud_tint(weather: Weather) -> f32 {
    let toward = |c: f32, grey: f32, amount: f32| c * (1.0 - amount) + c * grey * amount;
    let srgb = toward(toward(1.0, 0.6, weather.rain * 0.95), 0.2, weather.thunder * 0.95);
    srgb.powf(2.2)
}

/// Rain greys the sky toward 60% of its luminance, thunder toward 20%; a flash whitens it.
fn weathered(color: [f32; 3], weather: Weather) -> [f32; 3] {
    let toward = |c: [f32; 3], grey: f32, amount: f32| {
        let level = (c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11) * grey;
        c.map(|v| v * (1.0 - amount) + level * amount)
    };
    let c = toward(toward(color, 0.6, weather.rain * 0.75), 0.2, weather.thunder * 0.75);
    let flash = weather.flash.min(1.0) * 0.45;
    [c[0] * (1.0 - flash) + 0.8 * flash, c[1] * (1.0 - flash) + 0.8 * flash, c[2] * (1.0 - flash) + flash]
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
        let clear = Weather::default();
        let noon = Sky::at(NOON, clear);
        assert_eq!((noon.color, noon.darken, noon.turn, noon.stars, noon.celestial, noon.glow), (DAY, 0.0, 0.0, 0.0, 1.0, None));
        let midnight = Sky::at(18000.0, clear);
        assert_eq!((midnight.darken, midnight.stars), (NIGHT_DARKEN, 0.5));
        assert!(midnight.color[2] < 0.1 && (midnight.turn - 0.5).abs() < 1e-6, "{midnight:?}");
        // Dusk falls between sunset (12000) and 13670, when the last sky light is gone.
        let dusk = Sky::at(13000.0, clear);
        assert!(dusk.darken > 3.0 && dusk.darken < NIGHT_DARKEN, "{dusk:?}");
        assert_eq!(Sky::at(NOON + DAY_TICKS * 3.0, clear), noon, "the time wraps");
        assert_eq!((moon_phase(23999), moon_phase(24000), moon_phase(24000 * 9), moon_phase(-1)), (0, 1, 1, 7));
    }

    #[test]
    fn the_horizon_glows_at_sunset_and_sunrise() {
        // The sun is on the horizon at a quarter and three quarters of its turn: full strength, orange.
        let sunset = (0..24000).map(|t| Sky::at(t as f32, Weather::default())).min_by(|a, b| (a.turn - 0.25).abs().total_cmp(&(b.turn - 0.25).abs())).unwrap();
        let ([r, g, b, strength], side) = sunset.glow.expect("glow at sunset");
        assert!(strength > 0.99 && (r - 0.85).abs() < 0.01 && (g - 0.375).abs() < 0.01 && b == 0.2 && side == -1.0, "{:?}", sunset.glow);
        // Facing west the sky takes the glow's colour, facing east it keeps its own.
        assert!((sunset.color_towards(glam::Vec3::NEG_X)[0] - r).abs() < 0.01);
        assert_eq!(sunset.color_towards(glam::Vec3::X), sunset.color);
        assert_eq!(Sky::at(0.0, Weather::default()).glow.map(|g| g.1), Some(1.0), "sunrise is in the east");
    }

    #[test]
    fn storms_grey_and_darken_the_day() {
        let storm = Sky::at(NOON, Weather { rain: 1.0, thunder: 1.0, flash: 0.0 });
        // Java's sky light under a thunderstorm at noon: (1 - 11/16)² of the day, 11 levels × 0.902.
        assert!((storm.darken - NIGHT_DARKEN * (1.0 - (11.0f32 / 16.0).powi(2))).abs() < 1e-4, "{storm:?}");
        let [r, _, b] = storm.color;
        assert!((r - b).abs() < 0.12 && b < DAY[2] * 0.5, "{storm:?}");
        assert_eq!((storm.stars, storm.celestial), (0.0, 0.0));
        let flash = Sky::at(18000.0, Weather { flash: 1.0, ..Weather::default() });
        assert!(flash.color[2] > 0.45, "{flash:?}");
        // Java's storm clouds: 1 → 0.62 under rain → 0.149 under thunder (sRGB).
        let storm = cloud_tint(Weather { rain: 1.0, thunder: 1.0, flash: 0.0 }).powf(1.0 / 2.2);
        assert!((storm - 0.149).abs() < 0.002 && cloud_tint(Weather::default()) == 1.0, "{storm}");
    }
}
