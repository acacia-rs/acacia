//! Day, night and weather on the sky. A Java look samples 26.3's environment attributes
//! ([`environment`]); a look without them keeps the earlier formulas ([`legacy`]). Everything
//! here is CPU-side so it can be compared with the game's; the GPU passes only draw it.

pub mod attributes;
pub mod blend;
pub mod environment;
pub mod fog;
pub mod geometry;
mod joml;
mod legacy;
pub mod timeline;

use std::path::Path;

use image::RgbaImage;

use crate::assets::image_file;
pub use attributes::{Attr, Attributes};
pub use environment::{Environment, Skybox};

/// Ticks in a day; 0 is sunrise, 6000 noon, 18000 midnight.
pub const DAY_TICKS: f32 = 24000.0;
pub const NOON: f32 = 6000.0;
const MOON_PHASES: i64 = 8;

/// The weather the server reports.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Weather {
    /// 0 to 1.
    pub rain: f32,
    pub thunder: f32,
    /// A lightning flash, 0 to 1 (Java's `skyFlashTime` while above 0).
    pub flash: f32,
}

/// Which kind of sky a dimension has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Realm {
    Overworld,
    Nether,
    End,
}

impl Realm {
    /// `Dimension` carries no name: a dimension without sky light is the Nether when it is 128
    /// blocks tall, else the End.
    pub fn of(dimension: &acacia_world::Dimension) -> Realm {
        match (dimension.sky, dimension.height <= 128) {
            (true, _) => Realm::Overworld,
            (false, true) => Realm::Nether,
            (false, false) => Realm::End,
        }
    }

    /// Its dimension type's name in Java's data.
    pub fn java_name(self) -> &'static str {
        match self {
            Realm::Overworld => "overworld",
            Realm::Nether => "the_nether",
            Realm::End => "the_end",
        }
    }
}

/// The sky of one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sky {
    pub attributes: Attributes,
    pub skybox: Skybox,
    /// How visible the sun and moon are: rain hides them (`1 - rain`).
    pub celestial: f32,
}

impl Sky {
    /// Without environment attributes: the earlier formulas.
    pub fn of(realm: Realm, time: f32, weather: Weather) -> Sky {
        let skybox = match realm {
            Realm::Overworld => Skybox::Overworld,
            Realm::Nether => Skybox::None,
            Realm::End => Skybox::End,
        };
        let rain = if realm == Realm::Overworld { weather.rain } else { 0.0 };
        Sky { attributes: legacy::attributes(realm, time, weather), skybox, celestial: 1.0 - rain }
    }

    /// `ticks` on the dimension's clock; `biomes` the camera's ([`blend::gaussian_weights`], by
    /// Bedrock name). `None` when the environment lacks the dimension.
    pub fn sample(env: &Environment, realm: Realm, ticks: i64, biomes: &[(&str, f64)], weather: Weather) -> Option<Sky> {
        let dimension = env.dimension(realm.java_name())?;
        let attributes = env.sample(dimension, ticks, biomes, weather.rain, weather.thunder, weather.flash > 0.0);
        Some(Sky { attributes, skybox: dimension.skybox, celestial: 1.0 - weather.rain })
    }

    /// Sky light levels the time and weather take off.
    pub fn darken(&self) -> f32 {
        self.attributes.darken()
    }

    /// Sky colour (sRGB).
    pub fn color(&self) -> [f32; 3] {
        self.attributes.rgb(Attr::SkyColor)
    }

    /// What the cloud pass draws with: `visual/cloud_color` (sRGB, alpha its opacity, weather
    /// applied) and `visual/cloud_height`. Its fade distance is the frame fog's `cloud_end`.
    pub fn clouds(&self) -> CloudLook {
        CloudLook { color: self.attributes.get(Attr::CloudColor), height: self.attributes.number(Attr::CloudHeight) }
    }

    /// The fog colour seen looking along `forward`, pulled to the sunrise when facing it: the
    /// Bedrock look's simpler stand-in for [`fog::setup_fog`].
    pub fn color_towards(&self, forward: glam::Vec3) -> [f32; 3] {
        let [r, g, b, strength] = self.attributes.get(Attr::SunriseSunsetColor);
        let fog = self.attributes.rgb(Attr::FogColor);
        let side = if self.attributes.number(Attr::SunAngle).to_radians().sin() > 0.0 { -1.0 } else { 1.0 };
        let facing = (forward.x * side).max(0.0) * strength;
        let glow = [r, g, b];
        std::array::from_fn(|i| fog[i] * (1.0 - facing) + glow[i] * facing)
    }
}

/// [`Sky::clouds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudLook {
    pub color: [f32; 4],
    pub height: f32,
}

impl CloudLook {
    /// `layer` with this colour (as ARGB, `ARGB.colorFromFloat`) and height.
    pub fn layer(&self, layer: crate::clouds::CloudLayer) -> crate::clouds::CloudLayer {
        let [r, g, b, a] = self.color.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u32);
        crate::clouds::CloudLayer { color: a << 24 | r << 16 | g << 8 | b, height: self.height, ..layer }
    }
}

/// White clouds greyed by the weather (`ClientLevel.getCloudColor`), linear; for a look without
/// [`Attr::CloudColor`].
pub fn cloud_tint(weather: Weather) -> f32 {
    let toward = |c: f32, grey: f32, amount: f32| c * (1.0 - amount) + c * grey * amount;
    let srgb = toward(toward(1.0, 0.6, weather.rain * 0.95), 0.2, weather.thunder * 0.95);
    srgb.powf(2.2)
}

/// The moon's phase on the day a world time in ticks falls on: 0 is full, 4 new.
pub fn moon_phase(world_time: i64) -> u8 {
    world_time.div_euclid(DAY_TICKS as i64).rem_euclid(MOON_PHASES) as u8
}

/// `textures/environment/sun.png` and `moon_phases.png` (4×2 phases, in [`moon_phase`] order).
pub struct SkyTextures {
    pub sun: RgbaImage,
    pub moon: RgbaImage,
    /// `end_sky.png`, tiled over the End's sky; a pack without it gets a plain one.
    pub end: Option<RgbaImage>,
}

impl SkyTextures {
    pub fn load(root: &Path) -> Option<SkyTextures> {
        let open = |name: &str| Some(image::open(image_file(root, &format!("textures/environment/{name}"))?).ok()?.into_rgba8());
        Some(SkyTextures { sun: open("sun")?, moon: open("moon_phases")?, end: open("end_sky") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realms_and_their_still_skies() {
        let air = 0;
        let end = acacia_world::Dimension { min_y: 0, height: 256, air, sky: false };
        assert_eq!((Realm::of(&acacia_world::Dimension::overworld(air)), Realm::of(&acacia_world::Dimension::nether(air)), Realm::of(&end)), (Realm::Overworld, Realm::Nether, Realm::End));
        let storm = Weather { rain: 1.0, thunder: 1.0, flash: 1.0 };
        let nether = Sky::of(Realm::Nether, 18000.0, storm);
        assert_eq!((nether.skybox, nether.celestial, nether.color()), (Skybox::None, 1.0, legacy::NETHER_FOG));
        assert_eq!((moon_phase(23999), moon_phase(24000), moon_phase(24000 * 9), moon_phase(-1)), (0, 1, 1, 7));
    }

    #[test]
    fn the_sunset_side_tints_the_bedrock_fog() {
        let mut sky = Sky::of(Realm::Overworld, NOON, Weather::default());
        sky.attributes.set(Attr::SunriseSunsetColor, [1.0, 0.0, 0.0, 1.0]);
        sky.attributes.set(Attr::SunAngle, [90.0, 0.0, 0.0, 0.0]);
        assert_eq!(sky.color_towards(glam::Vec3::NEG_X), [1.0, 0.0, 0.0]);
        assert_eq!(sky.color_towards(glam::Vec3::X), sky.attributes.rgb(Attr::FogColor));
    }

    #[test]
    fn weather_layers_need_an_open_sky() {
        let doc = serde_json::json!({"dimensions": {"overworld": {"has_skylight": true}, "the_nether": {"has_ceiling": true, "skybox": "none"}}});
        let env = Environment::parse(&doc);
        let storm = Weather { rain: 1.0, thunder: 1.0, flash: 0.0 };
        let over = Sky::sample(&env, Realm::Overworld, 0, &[], storm).unwrap();
        assert!(over.darken() > 5.0, "{}", over.darken());
        let nether = Sky::sample(&env, Realm::Nether, 0, &[], storm).unwrap();
        assert_eq!((nether.darken(), nether.skybox), (0.0, Skybox::None));
    }
}
