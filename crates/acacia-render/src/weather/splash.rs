//! Rain splashes and the rain sound each tick: 26.3's `ClientLevel.tickWeatherEffects`.

use glam::DVec3;

use super::climate::Precipitation;
use super::columns::WeatherLevel;
use crate::java_random::JavaRandom;

pub trait SplashLevel: WeatherLevel {
    fn min_y(&self) -> i32;
    /// The higher of the block's collision top at `(x, z)` within it (`VoxelShape.max(Y, x, z)`)
    /// and its fluid's height, 0 to 1.
    fn splash_top(&self, p: [i32; 3], x: f64, z: f64) -> f64;
    /// Lava, a magma block or a lit campfire: it smokes instead of splashing.
    fn smokes(&self, p: [i32; 3]) -> bool;
}

/// The Particles option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleStatus {
    All,
    Decreased,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Splash {
    /// `ParticleTypes.SMOKE` rather than `RAIN`.
    pub smoke: bool,
    pub pos: DVec3,
}

/// `weather.rain.above` (0.1 volume, 0.5 pitch) under a roof, else `weather.rain` (0.2, 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RainSound {
    pub above: bool,
    pub at: [i32; 3],
}

/// The level's `rainSoundTime`, the only state that carries over between ticks.
#[derive(Debug, Default)]
pub struct RainTicker {
    sound_time: i32,
}

impl RainTicker {
    /// `rain` is `getRainLevel(1)`; `radius` the Weather Radius option.
    pub fn tick(&mut self, level: &impl SplashLevel, rain: f32, game_time: i64, camera: DVec3, radius: i32, status: ParticleStatus) -> (Vec<Splash>, Option<RainSound>) {
        let mut splashes = Vec::new();
        if rain <= 0.0 {
            return (splashes, None);
        }
        let mut random = JavaRandom::new(game_time.wrapping_mul(312_987_231));
        let [cx, cy, cz] = camera.floor().as_ivec3().to_array();
        let diameter = 2 * radius + 1;
        let area = diameter * diameter;
        let count = (0.225 * area as f32 * rain * rain) as i32 / if status == ParticleStatus::Decreased { 2 } else { 1 };
        let mut last = None;
        for _ in 0..count {
            let x = cx + random.next_int(diameter) - radius;
            let z = cz + random.next_int(diameter) - radius;
            let top = level.surface(x, z);
            if top <= level.min_y() || top > cy + 10 || top < cy - 10 || level.precipitation([x, top, z]) != Precipitation::Rain {
                continue;
            }
            let below = [x, top - 1, z];
            last = Some(below);
            if status == ParticleStatus::Minimal {
                break;
            }
            let (bx, bz) = (random.next_double(), random.next_double());
            let y = level.splash_top(below, bx, bz);
            let pos = DVec3::new(f64::from(x) + bx, f64::from(below[1]) + y, f64::from(z) + bz);
            splashes.push(Splash { smoke: level.smokes(below), pos });
        }
        let sound = last.filter(|_| {
            let due = random.next_int(3) < self.sound_time;
            self.sound_time += 1;
            due
        });
        let sound = sound.map(|at| {
            self.sound_time = 0;
            RainSound { above: at[1] > cy + 1 && level.surface(cx, cz) > cy, at }
        });
        (splashes, sound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ground(i32);

    impl WeatherLevel for Ground {
        fn surface(&self, x: i32, _: i32) -> i32 {
            if x == 0 { self.0 + 5 } else { self.0 }
        }
        fn precipitation(&self, _: [i32; 3]) -> Precipitation {
            Precipitation::Rain
        }
        fn light(&self, _: [i32; 3]) -> [u8; 2] {
            [0, 15]
        }
    }

    impl SplashLevel for Ground {
        fn min_y(&self) -> i32 {
            -64
        }
        fn splash_top(&self, _: [i32; 3], _: f64, _: f64) -> f64 {
            1.0
        }
        fn smokes(&self, p: [i32; 3]) -> bool {
            p[0] == 1
        }
    }

    #[test]
    fn splashes_land_on_the_ground_around_the_camera() {
        let mut ticker = RainTicker::default();
        let camera = DVec3::new(0.5, 70.0, 0.5);
        let (splashes, _) = ticker.tick(&Ground(64), 1.0, 77, camera, 10, ParticleStatus::All);
        // 0.225 × 21² = 99.2 tries, all landing.
        assert_eq!(splashes.len(), 99);
        assert!(splashes.iter().all(|s| (s.pos.x - camera.x).abs() <= 11.0 && (s.pos.y == 64.0 || s.pos.y == 69.0)));
        assert!(splashes.iter().any(|s| s.smoke) && splashes.iter().any(|s| !s.smoke));
        assert_eq!(ticker.tick(&Ground(64), 1.0, 77, camera, 10, ParticleStatus::Decreased).0.len(), 49);
        assert!(ticker.tick(&Ground(64), 1.0, 77, camera, 10, ParticleStatus::Minimal).0.is_empty());
        // Ground out of the camera's ten-block reach: none.
        assert!(ticker.tick(&Ground(90), 1.0, 77, camera, 10, ParticleStatus::All).0.is_empty());
    }

    #[test]
    fn the_rain_sound_comes_ever_more_likely_then_resets() {
        let mut ticker = RainTicker::default();
        let camera = DVec3::new(0.5, 70.0, 0.5);
        let sounds: Vec<bool> = (0..40).map(|t| ticker.tick(&Ground(64), 1.0, t, camera, 10, ParticleStatus::All).1.is_some()).collect();
        // nextInt(3) < time++: never on the first tick, always by the fourth since the last.
        assert!(!sounds[0] && sounds.iter().filter(|&&s| s).count() >= 10);
        assert!(sounds.windows(4).all(|w| w.iter().any(|&s| s)));
    }
}
