//! Day and night from the time of day, with Java's formulas (Bedrock's look the same).

use std::f32::consts::{PI, TAU};

/// Ticks in a day; 0 is sunrise, 6000 noon, 18000 midnight.
pub const DAY_TICKS: f32 = 24000.0;
pub const NOON: f32 = 6000.0;
/// Daytime sky and fog colour (sRGB).
const DAY: [f32; 3] = [0.62, 0.76, 1.0];
/// Sky light levels lost at midnight.
const NIGHT_DARKEN: f32 = 11.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sky {
    /// Sky and fog colour (sRGB).
    pub color: [f32; 3],
    /// Levels taken off every cell's sky light.
    pub darken: f32,
}

/// Height of the sun: 1 at noon, -1 at midnight. The sun lingers near noon and midnight.
fn sun_height(time: f32) -> f32 {
    let day = (time / DAY_TICKS - 0.25).rem_euclid(1.0);
    let angle = (day * 2.0 + 0.5 - (day * PI).cos() / 2.0) / 3.0;
    (angle * TAU).cos()
}

impl Sky {
    pub fn at(time: f32) -> Sky {
        let height = sun_height(time);
        let day = (height * 2.0 + 0.5).clamp(0.0, 1.0);
        let [r, g, b] = DAY;
        Sky {
            color: [r * (day * 0.94 + 0.06), g * (day * 0.94 + 0.06), b * (day * 0.91 + 0.09)],
            darken: (1.0 - (height * 2.0 + 0.2).clamp(0.0, 1.0)) * NIGHT_DARKEN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_bright_and_midnight_dark() {
        let noon = Sky::at(NOON);
        assert_eq!((noon.color, noon.darken), (DAY, 0.0));
        let midnight = Sky::at(18000.0);
        assert_eq!(midnight.darken, NIGHT_DARKEN);
        assert!(midnight.color[2] < 0.1, "{midnight:?}");
        // Dusk falls between sunset (12000) and 13670, when the last sky light is gone.
        let dusk = Sky::at(13000.0);
        assert!(dusk.darken > 3.0 && dusk.darken < NIGHT_DARKEN, "{dusk:?}");
        assert_eq!(Sky::at(NOON + DAY_TICKS * 3.0), noon, "the time wraps");
    }
}
