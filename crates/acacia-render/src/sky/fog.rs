//! Java's `FogRenderer.setupFog` and `computeFogColor` over its fog environments: atmospheric
//! (with rain fog), water (with water vision), lava, powder snow, blindness and darkness.

use super::attributes::{Attr, Attributes};

/// The camera's `FogType` (`Camera.getFluidInCamera`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogType {
    Atmospheric,
    Water,
    Lava,
    PowderSnow,
}

/// The fog uniform (`FogData`): sRGB colour, the environmental (spherical) and render-distance
/// (cylindrical) ramps, and where the sky and clouds are fully fogged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogData {
    pub color: [f32; 3],
    pub environmental_start: f32,
    pub environmental_end: f32,
    pub render_distance_start: f32,
    pub render_distance_end: f32,
    pub sky_end: f32,
    pub cloud_end: f32,
}

/// Everything `setupFog` reads for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogInputs {
    pub fog_type: FogType,
    pub render_distance_chunks: i32,
    /// The Cloud Distance option, in chunks.
    pub cloud_range_chunks: i32,
    pub camera_y: f64,
    pub min_y: i32,
    /// `voidDarknessOnsetRange`: 32 blocks over the floor, 1 in a flat world.
    pub void_onset: f32,
    pub forward: [f32; 3],
    pub rain: f32,
    pub thunder: f32,
    pub player: PlayerFog,
    /// [`water_vision`].
    pub water_vision: f32,
    /// [`RainFog::multiplier`].
    pub rain_fog: f32,
}

/// The player's effects and the boss bars, which the caller knows and the world does not.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayerFog {
    /// Blindness: (ticks left, infinite).
    pub blindness: Option<(i32, bool)>,
    /// The darkness effect's blend factor while it is on.
    pub darkness: Option<f32>,
    /// Night vision's scale (`GameRenderer.nightVisionScale`) while it is on.
    pub night_vision: Option<f32>,
    pub fire_resistance: bool,
    pub spectator: bool,
    /// `darkenWorldAmount`: a boss bar that darkens the screen (the Wither's), 0..1.
    pub darken_world: f32,
    /// A boss bar that creates world fog (the Ender Dragon's) is shown.
    pub boss_world_fog: bool,
}

const LAVA: [f32; 3] = rgb(0x991A00);
const POWDER_SNOW: [f32; 3] = rgb(0x9FBACC);

const fn rgb(c: u32) -> [f32; 3] {
    [(c >> 16 & 255) as f32 / 255.0, (c >> 8 & 255) as f32 / 255.0, (c & 255) as f32 / 255.0]
}

/// `LocalPlayer.getWaterVision` after `ticks` with the eye in water (`waterVisionTime`, 0..600).
pub fn water_vision(ticks: i32) -> f32 {
    if ticks >= 600 {
        return 1.0;
    }
    let t = ticks as f32;
    let late = if t < 100.0 { 0.0 } else { ((t - 100.0) / 500.0).clamp(0.0, 1.0) };
    (t / 100.0).clamp(0.0, 1.0) * 0.6 + late * 0.399_999_98
}

/// `GameRenderer.nightVisionScale`: full, flickering over its last 200 ticks. `ticks` -1 is infinite.
pub fn night_vision_scale(ticks: i32, partial: f32) -> f32 {
    if ticks == -1 || ticks > 200 {
        return 1.0;
    }
    0.7 + crate::java_mth::sin(f64::from((ticks as f32 - partial) * std::f32::consts::PI * 0.2)) * 0.3
}

/// What the fog carries from frame to frame, and what the caller tells it of the player.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Atmosphere {
    pub player: PlayerFog,
    pub rain_fog: RainFog,
    /// Game ticks at the last frame, for [`RainFog::update`].
    pub last_tick: Option<f64>,
}

/// `AtmosphericFogEnvironment.rainFogMultiplier`, eased each tick towards the rain the camera
/// stands in.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RainFog {
    pub multiplier: f32,
}

impl RainFog {
    /// `updateRainFogState` over `delta_ticks`: `sky_light` at the camera, `biome_rains` whether
    /// its biome has precipitation.
    pub fn update(&mut self, rain: f32, sky_light: u8, biome_rains: bool, delta_ticks: f32) {
        let sky = ((f32::from(sky_light) - 8.0) / 7.0).clamp(0.0, 1.0);
        let target = rain * sky * if biome_rains { 1.0 } else { 0.5 };
        self.multiplier += (target - self.multiplier) * delta_ticks * 0.2;
    }
}

/// `FogRenderer.setupFog` with `computeFogColor`'s colour.
pub fn setup_fog(a: &Attributes, input: &FogInputs) -> FogData {
    let rd = input.render_distance_chunks as f32 * 16.0;
    let mut color = match input.fog_type {
        FogType::Lava => LAVA,
        FogType::PowderSnow => POWDER_SNOW,
        FogType::Water => a.rgb(Attr::WaterFogColor),
        FogType::Atmospheric => atmospheric_color(a, input),
    };
    let range = input.void_onset;
    let mut darkness = ((range + input.min_y as f32 - input.camera_y as f32) / range).clamp(0.0, 1.0);
    if let Some((ticks, infinite)) = input.player.blindness {
        darkness = if !infinite && ticks < 20 { darkness.max(ticks as f32 / 20.0) } else { 1.0 };
    } else if let Some(blend) = input.player.darkness {
        darkness = darkness.max(blend);
    }
    if darkness > 0.0 && !matches!(input.fog_type, FogType::Lava | FogType::PowderSnow) {
        let brightness = (1.0 - darkness) * (1.0 - darkness);
        color = color.map(|c| c * brightness);
    }
    if input.player.darken_world > 0.0 {
        let scale = [0.7, 0.6, 0.6];
        color = std::array::from_fn(|i| color[i] + (color[i] * scale[i] - color[i]) * input.player.darken_world);
    }
    let brighten = match input.player.night_vision {
        _ if input.fog_type == FogType::Water => input.water_vision,
        Some(scale) if input.player.darkness.is_none() => scale,
        _ => 0.0,
    };
    if color.iter().all(|&c| c != 0.0) {
        let scale = 1.0 / color[0].max(color[1]).max(color[2]);
        color = color.map(|c| c + (c * scale - c) * brighten);
    }
    let (environmental_start, environmental_end, sky_end, cloud_end) = distances(a, input, rd);
    FogData {
        color,
        environmental_start,
        environmental_end,
        render_distance_start: rd - (rd / 10.0).clamp(4.0, 64.0),
        render_distance_end: rd,
        sky_end,
        cloud_end,
    }
}

/// `AtmosphericFogEnvironment.getBaseColor`: the fog colour turned to the sunrise when facing it,
/// with the sky colour mixed in more the shorter the render distance.
fn atmospheric_color(a: &Attributes, input: &FogInputs) -> [f32; 3] {
    let mut fog = a.rgb(Attr::FogColor);
    if input.render_distance_chunks >= 4 {
        let sun_side = if crate::java_mth::sin(f64::from(super::geometry::radians(a.number(Attr::SunAngle)))) > 0.0 { -1.0 } else { 1.0 };
        let looking = input.forward[0] * sun_side;
        let sunrise = a.get(Attr::SunriseSunsetColor);
        if looking > 0.0 && sunrise[3] > 0.0 {
            let t = looking * sunrise[3];
            fog = std::array::from_fn(|i| fog[i] + (sunrise[i] - fog[i]) * t);
        }
    }
    let mut sky = a.rgb(Attr::SkyColor);
    if input.rain > 0.0 {
        let (m, mb) = (1.0 - input.rain * 0.5, 1.0 - input.rain * 0.4);
        sky = [sky[0] * m, sky[1] * m, sky[2] * mb];
    }
    if input.thunder > 0.0 {
        sky = sky.map(|c| c * (1.0 - input.thunder * 0.5));
    }
    let sky_fog_end = (a.number(Attr::SkyFogEnd) / 16.0).min(input.render_distance_chunks as f32);
    let mix = 1.0 - (sky_fog_end / 32.0).clamp(0.0, 1.0).mul_add(0.75, 0.25).powf(0.25);
    std::array::from_fn(|i| fog[i] + (sky[i] - fog[i]) * mix)
}

/// (environmental start, end, sky end, cloud end) of the environment that applies.
fn distances(a: &Attributes, input: &FogInputs, rd: f32) -> (f32, f32, f32, f32) {
    match input.fog_type {
        FogType::Lava | FogType::PowderSnow if input.player.spectator => (-8.0, rd * 0.5, rd * 0.5, rd * 0.5),
        FogType::Lava if input.player.fire_resistance => (0.0, 5.0, 5.0, 5.0),
        FogType::Lava => (0.25, 1.0, 1.0, 1.0),
        FogType::PowderSnow => (0.0, 2.0, 2.0, 2.0),
        _ if input.player.blindness.is_some() => {
            let (ticks, infinite) = input.player.blindness.unwrap_or((0, true));
            let d = if infinite { 5.0 } else { rd + (5.0 - rd) * (ticks as f32 / 20.0).min(1.0) };
            (d * 0.25, d, d * 0.8, d * 0.8)
        }
        _ if input.player.darkness.is_some() => {
            let d = rd + (15.0 - rd) * input.player.darkness.unwrap_or(0.0);
            (d * 0.75, d, d, d)
        }
        FogType::Water => {
            let end = a.number(Attr::WaterFogEnd) * input.water_vision.max(0.25);
            (a.number(Attr::WaterFogStart), end, end, end)
        }
        FogType::Atmospheric => {
            let (fog_start, fog_end) = (a.number(Attr::FogStart), a.number(Attr::FogEnd));
            let start = fog_start - 160.0 * input.rain_fog;
            let end = fog_end.min(96.0).max(fog_end - 256.0 * input.rain_fog);
            let cloud_end = ((input.cloud_range_chunks * 16) as f32).min(a.number(Attr::CloudFogEnd));
            if input.player.boss_world_fog {
                let end = end.min(96.0);
                (start.min(10.0), end, end, end)
            } else {
                (start, end, rd.min(a.number(Attr::SkyFogEnd)), cloud_end)
            }
        }
    }
}

#[cfg(test)]
mod tests;
