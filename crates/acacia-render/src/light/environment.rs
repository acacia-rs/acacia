//! The frame's lightmap from the dimension, the time of day, the weather, the brightness option
//! and the effects on the player: Java's `LightmapRenderStateExtractor` over the light attributes
//! of its `EnvironmentAttributeSystem` (dimension, `day` timeline, `WeatherAttributes`, the
//! lightning layer of `ClientLevel`). No biome sets a light attribute in 26.3.

use super::lightmap::{LightmapState, Table, curve_table};
use crate::look::Lighting;
use crate::sky::{Realm, Sky, Weather};

/// What the player brings to the lightmap; the caller keeps it up to date.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightConditions {
    /// The brightness option, 0 (moody) to 1 (bright).
    pub gamma: f32,
    /// Night vision, 0 to 1: `GameRenderer.nightVisionScale`, or the water vision under conduit power.
    pub night_vision: f32,
    /// The darkness effect's blend factor times the darkness effect scale option.
    pub darkness: f32,
    /// The darkness effect scale option.
    pub darkness_scale: f32,
    /// The player's age in ticks, with the partial tick, for the darkness pulse.
    pub age: f32,
    /// `GameRenderer.bossOverlayWorldDarkening`.
    pub boss_darkening: f32,
    /// The End's flash, 0 to 1, and whether a boss bar fogs the world (a third of it shows then).
    pub end_flash: (f32, bool),
    /// The torch flicker ([`Flicker`]); the renderer draws with its own, whatever this holds.
    pub flicker: f32,
}

impl Default for LightConditions {
    fn default() -> Self {
        LightConditions { gamma: 0.5, night_vision: 0.0, darkness: 0.0, darkness_scale: 1.0, age: 0.0, boss_darkening: 0.0, end_flash: (0.0, false), flicker: 0.0 }
    }
}

/// Brightness of unlit blocks under the renderer's own curve, with and without sky light.
const CURVE_AMBIENT: (f32, f32) = (0.02, 0.1);

pub fn frame_lightmap(lighting: Lighting, realm: Realm, time: f32, weather: Weather, c: &LightConditions) -> Table {
    match lighting {
        Lighting::Volume => {
            let ambient = if realm == Realm::Overworld { CURVE_AMBIENT.0 } else { CURVE_AMBIENT.1 };
            curve_table(ambient, Sky::of(realm, time, weather).darken)
        }
        Lighting::Java => java_state(realm, time, weather, c).table(),
    }
}

/// `ARGB.vector3fFromRGB24`.
const fn rgb(c: u32) -> [f32; 3] {
    [(c >> 16 & 255) as f32 / 255.0, (c >> 8 & 255) as f32 / 255.0, (c & 255) as f32 / 255.0]
}

/// The light attributes the lightmap reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightAttributes {
    pub sky_factor: f32,
    pub sky_color: [f32; 3],
    pub block_tint: [f32; 3],
    pub ambient: [f32; 3],
    pub night_vision_color: [f32; 3],
}

impl LightAttributes {
    /// The attributes' defaults with the dimension type's own (`dimension_type/*.json`).
    pub fn of_dimension(realm: Realm) -> LightAttributes {
        let base = LightAttributes { sky_factor: 1.0, sky_color: rgb(0xffffff), block_tint: rgb(0xffd88c), ambient: rgb(0x0a0a0a), night_vision_color: rgb(0x999999) };
        match realm {
            Realm::Overworld => base,
            Realm::Nether => LightAttributes { sky_factor: 0.0, sky_color: rgb(0x7a7aff), ambient: rgb(0x302821), ..base },
            Realm::End => LightAttributes { sky_factor: 0.0, sky_color: rgb(0xac60cd), ambient: rgb(0x3f473f), ..base },
        }
    }

    /// At `time` (the overworld clock, in ticks) and in `weather`: only the overworld has the day
    /// timeline and weather.
    pub fn at(realm: Realm, time: f32, weather: Weather) -> LightAttributes {
        let mut a = LightAttributes::of_dimension(realm);
        if realm == Realm::Overworld {
            let ticks = (time.floor() as i64).rem_euclid(DAY_PERIOD);
            a.sky_factor *= sample(&SKY_FACTOR_TRACK, ticks, |t, x: &f32, y: &f32| lerp(t, *x, *y));
            let color = sample(&SKY_COLOR_TRACK, ticks, |t, x: &[f32; 3], y: &[f32; 3]| lerp3(t, *x, *y));
            a.sky_color = [0, 1, 2].map(|i| a.sky_color[i] * color[i]);
            a.apply_weather(weather.rain, weather.thunder);
        }
        if weather.flash > 0.0 {
            a.sky_factor = 1.0;
        }
        a.sky_factor = a.sky_factor.clamp(0.0, 1.0);
        a
    }

    /// `WeatherAttributes`: the rain layer over what thunder leaves, then the thunder layer.
    fn apply_weather(&mut self, rain: f32, thunder: f32) {
        for (level, alpha) in [(rain - thunder, 0.3125f32), (thunder, 0.527_343_75)] {
            if level <= 0.0 {
                continue;
            }
            let factor = lerp(alpha, self.sky_factor, 0.24);
            self.sky_factor = lerp(level, self.sky_factor, factor);
            // The colour's alpha goes through a byte (`ARGB.color(float, int)`).
            let colour_alpha = (alpha * 255.0).floor() / 255.0;
            let night = lerp3(colour_alpha, self.sky_color, NIGHT_SKY_COLOR);
            self.sky_color = lerp3(level, self.sky_color, night);
        }
    }
}

/// `Timelines.NIGHT_SKY_LIGHT_COLOR`.
const NIGHT_SKY_COLOR: [f32; 3] = rgb(0x7a7aff);
const DAY_PERIOD: i64 = 24000;
/// The `day` timeline's `visual/sky_light_factor` and `visual/sky_light_color`, both multiplying
/// and eased linearly.
const SKY_FACTOR_TRACK: [(i64, f32); 4] = [(730, 1.0), (11270, 1.0), (13140, 0.24), (22860, 0.24)];
const SKY_COLOR_TRACK: [(i64, [f32; 3]); 4] = [(730, rgb(0xffffff)), (11270, rgb(0xffffff)), (13140, NIGHT_SKY_COLOR), (22860, NIGHT_SKY_COLOR)];

/// `KeyframeTrackSampler.sample` of a looping track with a linear ease.
fn sample<T: Copy>(keys: &[(i64, T)], ticks: i64, lerp: impl Fn(f32, &T, &T) -> T) -> T {
    let (first, last) = (keys[0], keys[keys.len() - 1]);
    let mut segments = vec![(last, last.0 - DAY_PERIOD, first, first.0)];
    segments.extend(keys.windows(2).map(|w| (w[0], w[0].0, w[1], w[1].0)));
    segments.push((last, last.0, first, first.0 + DAY_PERIOD));
    let &(from, from_ticks, to, to_ticks) = segments.iter().find(|s| ticks < s.3).unwrap_or(&segments[segments.len() - 1]);
    if ticks <= from_ticks {
        return from.1;
    }
    if ticks >= to_ticks {
        return to.1;
    }
    lerp((ticks - from_ticks) as f32 / (to_ticks - from_ticks) as f32, &from.1, &to.1)
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

/// JOML's `Vector3f.lerp`, which fuses the multiply and add.
fn lerp3(t: f32, a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| (b[i] - a[i]).mul_add(t, a[i]))
}

/// Java's `LightmapRenderStateExtractor.extract`.
pub fn java_state(realm: Realm, time: f32, weather: Weather, c: &LightConditions) -> LightmapState {
    let a = LightAttributes::at(realm, time, weather);
    let (intensity, fogged) = c.end_flash;
    let end_flash = if fogged { intensity / 3.0 } else { intensity };
    let pulse = (mth_cos(c.age * std::f32::consts::PI * 0.025) * (0.45 * c.darkness)).max(0.0);
    LightmapState {
        sky_factor: a.sky_factor + end_flash,
        sky_color: a.sky_color,
        block_factor: c.flicker + 1.4,
        block_tint: a.block_tint,
        ambient: a.ambient,
        night_vision: c.night_vision,
        night_vision_color: a.night_vision_color,
        darkness: pulse * c.darkness_scale,
        brightness: (c.gamma - c.darkness).max(0.0),
        boss_darkening: c.boss_darkening,
    }
}

/// `Mth.cos`: a 65536-entry sine table.
fn mth_cos(x: f32) -> f32 {
    const STEPS: f64 = 10430.378350470453;
    let i = (f64::from(x) * STEPS + 16384.0) as i64 & 65535;
    (i as f64 / STEPS).sin() as f32
}

/// The torch flicker: a random walk stepped once a tick. Java seeds it at random, so any seed is
/// the game's.
#[derive(Debug, Clone, Default)]
pub struct Flicker {
    value: f32,
    seed: u64,
    tick: u64,
}

impl Flicker {
    /// The flicker at game tick `tick`, stepping every tick since the last call.
    pub fn at(&mut self, tick: u64) -> f32 {
        // A long pause would step for ages; the walk forgets its past within a few dozen steps.
        let steps = tick.saturating_sub(self.tick).min(64);
        self.tick = tick;
        for _ in 0..steps {
            let [a, b, c, d] = [0; 4].map(|_| self.next_float());
            self.value += (a - b) * c * d * 0.1;
            self.value *= 0.9;
        }
        self.value
    }

    /// splitmix64 to a float in 0..1.
    fn next_float(&mut self) -> f32 {
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_day_timeline_dims_the_sky_at_night() {
        let clear = Weather::default();
        let noon = LightAttributes::at(Realm::Overworld, 6000.0, clear);
        assert_eq!((noon.sky_factor, noon.sky_color), (1.0, [1.0; 3]));
        let night = LightAttributes::at(Realm::Overworld, 18000.0, clear);
        assert_eq!((night.sky_factor, night.sky_color), (0.24, NIGHT_SKY_COLOR));
        let dusk = LightAttributes::at(Realm::Overworld, 12205.0, clear);
        assert!((dusk.sky_factor - 0.62).abs() < 1e-3, "{dusk:?}");
        // Midnight wraps: 22860 to 730 the next day is a segment of its own.
        let dawn = LightAttributes::at(Realm::Overworld, 24000.0 + 12.0, clear);
        assert!(dawn.sky_factor > 0.24 && dawn.sky_factor < 1.0);
    }

    #[test]
    fn rain_and_lightning_change_the_sky_light() {
        let rain = LightAttributes::at(Realm::Overworld, 6000.0, Weather { rain: 1.0, thunder: 0.0, flash: 0.0 });
        assert!((rain.sky_factor - lerp(0.3125, 1.0, 0.24)).abs() < 1e-6);
        let flash = LightAttributes::at(Realm::Overworld, 18000.0, Weather { rain: 1.0, thunder: 1.0, flash: 0.5 });
        assert_eq!(flash.sky_factor, 1.0);
        let nether = LightAttributes::at(Realm::Nether, 6000.0, Weather { rain: 1.0, thunder: 1.0, flash: 0.0 });
        assert_eq!((nether.sky_factor, nether.ambient), (0.0, rgb(0x302821)));
    }

    #[test]
    fn the_flicker_stays_small() {
        let mut f = Flicker::default();
        let walk: Vec<f32> = (0..200).map(|t| f.at(t)).collect();
        assert!(walk.iter().all(|v| v.abs() < 0.1) && walk.iter().any(|v| *v != 0.0));
    }
}
