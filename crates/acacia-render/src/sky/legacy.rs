//! The sky before 26's timelines (`getTimeOfDay`, `getSkyColor`, `getSunriseColor`), for a look
//! without `environment.json`: Bedrock's, and Java looks baked before it.

use std::f32::consts::{PI, TAU};

use super::attributes::{Attr, Attributes};
use super::{DAY_TICKS, Realm, Weather};

/// Daytime sky and fog colour (sRGB).
pub(super) const DAY: [f32; 3] = [0.62, 0.76, 1.0];
/// Sky light levels lost at midnight.
pub(super) const NIGHT_DARKEN: f32 = 11.0;
/// Java's fog colours where there is no day: the nether wastes', and the End's 0xA080A0 at its
/// 0.15 brightness.
pub(super) const NETHER_FOG: [f32; 3] = [0.2, 0.031, 0.031];
pub(super) const END_FOG: [f32; 3] = [0.094, 0.075, 0.094];

pub(super) fn attributes(realm: Realm, time: f32, weather: Weather) -> Attributes {
    let mut a = Attributes::default();
    let still = |a: &mut Attributes, c: [f32; 3]| {
        a.set(Attr::SkyColor, [c[0], c[1], c[2], 1.0]);
        a.set(Attr::FogColor, [c[0], c[1], c[2], 1.0]);
    };
    match realm {
        Realm::Nether => still(&mut a, NETHER_FOG),
        Realm::End => still(&mut a, END_FOG),
        Realm::Overworld => {
            let day = (time / DAY_TICKS - 0.25).rem_euclid(1.0);
            let turn = (day * 2.0 + 0.5 - (day * PI).cos() / 2.0) / 3.0;
            let height = (turn * TAU).cos();
            let light = (height * 2.0 + 0.5).clamp(0.0, 1.0);
            let stars = (0.75 - height * 2.0).clamp(0.0, 1.0);
            let daylight = (height * 2.0 + 0.2).clamp(0.0, 1.0) * (1.0 - weather.rain * 5.0 / 16.0) * (1.0 - weather.thunder * 5.0 / 16.0);
            let [r, g, b] = DAY;
            let color = weathered([r * (light * 0.94 + 0.06), g * (light * 0.94 + 0.06), b * (light * 0.91 + 0.09)], weather);
            still(&mut a, color);
            let angle = turn * 360.0;
            a.set(Attr::SunAngle, [angle, 0.0, 0.0, 0.0]);
            a.set(Attr::MoonAngle, [angle + 180.0, 0.0, 0.0, 0.0]);
            a.set(Attr::StarAngle, [angle, 0.0, 0.0, 0.0]);
            a.set(Attr::StarBrightness, [stars * stars * 0.5 * (1.0 - weather.rain), 0.0, 0.0, 0.0]);
            a.set(Attr::SunriseSunsetColor, sunrise(height).unwrap_or_default());
            a.set(Attr::SkyLightLevel, [15.0 - (1.0 - daylight) * NIGHT_DARKEN, 0.0, 0.0, 0.0]);
            let grey = super::cloud_tint(weather).powf(1.0 / 2.2);
            a.set(Attr::CloudColor, [grey, grey, grey, 0.8]);
        }
    }
    a
}

/// `getSunriseOrSunsetColor`: `height` is the sun's (1 at noon, 0 on the horizon).
fn sunrise(height: f32) -> Option<[f32; 4]> {
    if !(-0.4..=0.4).contains(&height) {
        return None;
    }
    let h = height / 0.4 * 0.5 + 0.5;
    let strength = (1.0 - (1.0 - (h * PI).sin()) * 0.99).powi(2);
    Some([h * 0.3 + 0.7, h * h * 0.7 + 0.2, 0.2, strength])
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

#[cfg(test)]
mod tests {
    use super::super::NOON;
    use super::*;

    fn at(time: f32, weather: Weather) -> Attributes {
        attributes(Realm::Overworld, time, weather)
    }

    #[test]
    fn noon_is_bright_and_midnight_dark() {
        let noon = at(NOON, Weather::default());
        assert_eq!((noon.rgb(Attr::SkyColor), noon.darken(), noon.number(Attr::SunAngle), noon.number(Attr::StarBrightness)), (DAY, 0.0, 0.0, 0.0));
        let midnight = at(18000.0, Weather::default());
        assert_eq!((midnight.darken(), midnight.number(Attr::StarBrightness)), (NIGHT_DARKEN, 0.5));
        assert!((midnight.number(Attr::SunAngle) - 180.0).abs() < 1e-3);
        let dusk = at(13000.0, Weather::default());
        assert!(dusk.darken() > 3.0 && dusk.darken() < NIGHT_DARKEN);
        assert_eq!(at(NOON + DAY_TICKS * 3.0, Weather::default()), noon, "the time wraps");
    }

    #[test]
    fn the_horizon_glows_at_sunset() {
        let sunset = (0..24000).map(|t| at(t as f32, Weather::default())).min_by(|a, b| (a.number(Attr::SunAngle) - 90.0).abs().total_cmp(&(b.number(Attr::SunAngle) - 90.0).abs())).unwrap();
        let [r, g, b, strength] = sunset.get(Attr::SunriseSunsetColor);
        assert!(strength > 0.99 && (r - 0.85).abs() < 0.01 && (g - 0.375).abs() < 0.01 && b == 0.2);
    }

    #[test]
    fn storms_darken_and_the_nether_stays_still() {
        let storm = at(NOON, Weather { rain: 1.0, thunder: 1.0, flash: 0.0 });
        assert!((storm.darken() - NIGHT_DARKEN * (1.0 - (11.0f32 / 16.0).powi(2))).abs() < 1e-4);
        assert_eq!(storm.number(Attr::StarBrightness), 0.0);
        let storm = Weather { rain: 1.0, thunder: 1.0, flash: 1.0 };
        for (realm, color) in [(Realm::Nether, NETHER_FOG), (Realm::End, END_FOG)] {
            let a = attributes(realm, 18000.0, storm);
            assert_eq!((a.rgb(Attr::SkyColor), a.darken(), a.number(Attr::StarBrightness)), (color, 0.0, 0.0));
        }
    }
}
