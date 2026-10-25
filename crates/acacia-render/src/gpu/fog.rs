//! The sky and fog of a frame. The Java look samples the environment attributes (its
//! `environment.json`, else the earlier formulas) and runs Java's fog rules ([`crate::sky::fog`]);
//! the Bedrock look keeps its haze and the pack's fluid fogs.

use acacia_world::World;
use glam::DVec3;

use super::Renderer;
use super::globals::srgb_to_linear;
use crate::biome::BiomeColors;
use crate::camera::Camera;
use crate::fluid_view::{self, Fluid};
use crate::look::FluidFog;
use crate::sky::fog::{FogInputs, FogType, setup_fog, water_vision};
use crate::sky::{Attr, DAY_TICKS, Realm, Sky, Weather, blend};

/// Java's default Cloud Distance option, in chunks.
const CLOUD_RANGE_CHUNKS: i32 = 128;
const VOID_DARKNESS_ONSET: f32 = 32.0;

pub(super) struct FrameFog {
    /// Linear rgb: the fog's, and the clear colour.
    pub color: [f32; 3],
    /// Where the second, spherical fog starts and is opaque ([`super::globals::Globals::new`]).
    pub haze: Option<(f32, f32)>,
    /// The camera is in a fluid: no sky, sun or clouds.
    pub in_fluid: bool,
    pub underwater: bool,
    /// Blocks out where the sky and the clouds are fully fogged (`FogData.skyEnd`, `cloudEnd`).
    pub sky_end: f32,
    // TODO: the cloud pass (the weather fork's) fades clouds out by this.
    #[allow(dead_code)]
    pub cloud_end: f32,
}

impl Renderer {
    /// The sky the camera sees and the fog it draws with; advances the rain fog.
    pub(super) fn frame_environment(&mut self, camera: &Camera) -> (Sky, FrameFog) {
        let world = self.world().cloned();
        let realm = world.as_ref().map_or(Realm::Overworld, |w| Realm::of(&w.dimension()));
        // The server's rain level outlasts a trip to the Nether or End.
        let open_sky = world.as_ref().is_none_or(|w| w.dimension().sky);
        let weather = if open_sky { self.weather } else { Weather::default() };
        let sky = self.sample_sky(world.as_deref(), realm, camera.position, weather);
        let fog = match self.look.fluid_fog {
            FluidFog::Java => self.java_fog(world.as_deref(), &sky, camera, weather),
            FluidFog::Pack => self.pack_fog(&sky, camera),
        };
        (sky, fog)
    }

    fn sample_sky(&self, world: Option<&World>, realm: Realm, eye: DVec3, weather: Weather) -> Sky {
        let ticks = i64::from(self.moon_phase) * DAY_TICKS as i64 + self.time.floor() as i64;
        let legacy = || {
            let mut sky = Sky::of(realm, self.time, weather);
            sky.attributes.set(Attr::MoonPhase, [f32::from(self.moon_phase), 0.0, 0.0, 0.0]);
            sky
        };
        let Some(env) = self.biomes.environment() else { return legacy() };
        let weights = world.map_or_else(Vec::new, |w| biome_weights(w, &self.biomes, eye));
        Sky::sample(env, realm, ticks, &weights, weather).unwrap_or_else(legacy)
    }

    fn java_fog(&mut self, world: Option<&World>, sky: &Sky, camera: &Camera, weather: Weather) -> FrameFog {
        let eye = self.in_fluid;
        let mut attributes = sky.attributes;
        if self.biomes.environment().is_none() {
            // A look without environment.json: the pack's water fog per biome, as before it.
            let water = &self.biomes.fluid_fogs().water(eye.and_then(|e| e.biome)).fog;
            let scale = if water.relative { self.fog_distance } else { 1.0 };
            let [r, g, b] = water.color.map(|c| f32::from(c) / 255.0);
            attributes.set(Attr::WaterFogColor, [r, g, b, 1.0]);
            attributes.set(Attr::WaterFogEnd, [water.end * scale, 0.0, 0.0, 0.0]);
        }
        let rain_fog = self.update_rain_fog(camera, weather.rain);
        let input = FogInputs {
            fog_type: eye.map_or(FogType::Atmospheric, |e| e.fog_type()),
            render_distance_chunks: (self.fog_distance / 16.0).round() as i32,
            cloud_range_chunks: CLOUD_RANGE_CHUNKS,
            camera_y: camera.position.y,
            min_y: world.map_or(0, |w| w.dimension().min_y),
            void_onset: VOID_DARKNESS_ONSET,
            forward: camera.forward().to_array(),
            rain: weather.rain,
            thunder: weather.thunder,
            player: self.atmosphere.player,
            water_vision: eye.filter(|e| e.fluid == Fluid::Water).map_or(0.0, |e| water_vision(e.water_vision_ticks())),
            rain_fog,
        };
        let fog = setup_fog(&attributes, &input);
        FrameFog {
            color: srgb_to_linear(fog.color),
            haze: Some((fog.environmental_start, fog.environmental_end)),
            in_fluid: eye.is_some(),
            underwater: eye.is_some_and(|e| e.fluid == Fluid::Water),
            sky_end: fog.sky_end,
            cloud_end: fog.cloud_end,
        }
    }

    fn update_rain_fog(&mut self, camera: &Camera, rain: f32) -> f32 {
        let now = self.started.elapsed().as_secs_f64() * 20.0;
        let delta = self.atmosphere.last_tick.map_or(0.0, |last| (now - last) as f32);
        self.atmosphere.last_tick = Some(now);
        let light_data = self.scene.as_ref().map(|s| s.light().clone());
        let sky_light = self.light_cache.reader(light_data.as_deref()).at(camera.position)[1] as u8;
        let rains = camera.biome.is_none_or(|id| self.biomes.climate(id).has_precipitation);
        self.atmosphere.rain_fog.update(rain, sky_light, rains, delta);
        self.atmosphere.rain_fog.multiplier
    }

    fn pack_fog(&self, sky: &Sky, camera: &Camera) -> FrameFog {
        let ends = |end: f32| (end, end);
        if let Some(eye) = &self.in_fluid {
            // The fluid's fog stands in for the look's haze, so it is linear and spherical.
            let v = fluid_view::view(self.biomes.fluid_fogs(), eye, self.fog_distance);
            let (sky_end, cloud_end) = ends(v.end);
            return FrameFog { color: srgb_to_linear(v.color), haze: Some((v.start, v.end)), in_fluid: true, underwater: eye.fluid == Fluid::Water, sky_end, cloud_end };
        }
        // Only the Nether is that low; the End hazes as the overworld does.
        let nether = self.world().is_some_and(|w| !w.dimension().sky && w.dimension().height <= 128);
        let haze = self.look.fog.haze.map(|h| if nether { h.nether } else { h.overworld });
        let own = self.biomes.fluid_fogs().air(camera.biome).filter(|_| nether).map(|c| c.map(|v| f32::from(v) / 255.0));
        let (sky_end, cloud_end) = ends(self.fog_distance);
        FrameFog { color: srgb_to_linear(own.unwrap_or_else(|| sky.color_towards(camera.forward()))), haze, in_fluid: false, underwater: false, sky_end, cloud_end }
    }
}

/// The camera's biomes by name with their weights ([`blend::gaussian_weights`] over the world's
/// biome cells); a cell in an unloaded chunk weighs as no biome.
fn biome_weights<'a>(world: &World, biomes: &'a BiomeColors, eye: DVec3) -> Vec<(&'a str, f64)> {
    let at = |x: i32, y: i32, z: i32| {
        let (bx, by, bz) = (x * 4, y * 4, z * 4);
        world.get(bx >> 4, bz >> 4).and_then(|c| c.read().biome(bx, by, bz))
    };
    blend::gaussian_weights(eye.to_array(), at).into_iter().map(|(id, w)| (id.and_then(|id| biomes.name(id)).unwrap_or(""), w)).collect()
}
