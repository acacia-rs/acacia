//! What the renderer's sky shows each frame: the weather, lightning, clouds and the time of day.

use acacia_render::clouds::CloudLayer;
use acacia_render::sky::{DAY_TICKS, Weather, moon_phase};

use super::App;

/// Java's default Cloud Distance, in chunks.
const CLOUD_RANGE: i32 = 128;
/// Share of the gap to the server's time of day closed per second.
const TIME_EASE: f32 = 3.0;

impl App {
    pub(super) fn feed_sky(&mut self, dt: f32) {
        let Some(r) = &mut self.renderer else { return };
        let me = self.play.me.as_ref();
        r.weather = me.map_or_else(Weather::default, |m| Weather { rain: m.rain, thunder: m.thunder, flash: if m.bolts.is_empty() { 0.0 } else { 1.0 } });
        r.bolts = me.map_or_else(Vec::new, |m| m.bolts.clone());
        // Colour, height and fog end are the environment's; the renderer fills them in each frame.
        r.clouds = self.settings.clouds.then_some(CloudLayer { height: 0.0, color: 0, fancy: self.settings.fancy_clouds, range: CLOUD_RANGE, fog_end: 0.0 });
        if let Some(time) = self.time {
            // The server sends the time every few seconds: ease towards it, the short way round the day.
            let ahead = (time as f32 - r.time + DAY_TICKS / 2.0).rem_euclid(DAY_TICKS) - DAY_TICKS / 2.0;
            r.time += ahead * (dt * TIME_EASE).min(1.0);
            r.moon_phase = moon_phase(i64::from(time));
        }
    }
}
