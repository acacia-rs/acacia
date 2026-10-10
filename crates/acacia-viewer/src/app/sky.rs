//! What the renderer's sky shows each frame: the weather, lightning, clouds and the time of day.

use acacia_render::clouds::CloudLayer;
use acacia_render::sky::{DAY_TICKS, Weather, moon_phase};

use super::App;

/// Java's cloud height; Bedrock's is unmeasured, so both looks use it.
const CLOUD_HEIGHT: f32 = 192.33;
/// Share of the gap to the server's time of day closed per second.
const TIME_EASE: f32 = 3.0;

impl App {
    pub(super) fn feed_sky(&mut self, dt: f32) {
        let Some(r) = &mut self.renderer else { return };
        let me = self.play.me.as_ref();
        r.weather = me.map_or_else(Weather::default, |m| Weather { rain: m.rain, thunder: m.thunder, flash: if m.bolts.is_empty() { 0.0 } else { 1.0 } });
        r.bolts = me.map_or_else(Vec::new, |m| m.bolts.clone());
        r.clouds = self.settings.clouds.then_some(CloudLayer { height: CLOUD_HEIGHT, fancy: self.settings.fancy_clouds });
        if let Some(time) = self.time {
            // The server sends the time every few seconds: ease towards it, the short way round the day.
            let ahead = (time as f32 - r.time + DAY_TICKS / 2.0).rem_euclid(DAY_TICKS) - DAY_TICKS / 2.0;
            r.time += ahead * (dt * TIME_EASE).min(1.0);
            r.moon_phase = moon_phase(i64::from(time));
        }
    }
}
