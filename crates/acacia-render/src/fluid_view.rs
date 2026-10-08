//! The fog with the camera in water, lava or powder snow, by the look's [`FluidFog`] rules.

use crate::biome::fog::{Fog, FluidFogs};
use crate::look::FluidFog;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fluid {
    Water,
    Lava,
    PowderSnow,
}

/// What the camera is in, set by the caller each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InFluid {
    pub fluid: Fluid,
    /// Biome id at the camera, if known.
    pub biome: Option<u32>,
    /// Since the camera entered this fluid.
    pub seconds: f32,
}

/// The fog drawn instead of the sky's: sRGB colour, opaque at `end` blocks from the camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidView {
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
}

/// Java's `WaterFogEnvironment` reach at full water vision, and `LavaFogEnvironment` and
/// `PowderedSnowFogEnvironment` (not spectating, no fire resistance).
const JAVA_LAVA: Fog = Fog { color: [0x99, 0x1A, 0x00], start: 0.25, end: 1.0, relative: false };
const JAVA_POWDER_SNOW: Fog = Fog { color: [0x9F, 0xBB, 0xC8], start: 0.0, end: 2.0, relative: false };

/// Java's `textures/misc/underwater.png`, when the look has it (Bedrock draws no overlay).
pub fn load_underwater(files: &std::path::Path) -> Option<image::RgbaImage> {
    Some(image::open(crate::assets::image_file(files, "textures/misc/underwater")?).ok()?.into_rgba8())
}

/// `render_distance` is in blocks, for fogs given as a share of it.
pub fn view(rule: FluidFog, fogs: &FluidFogs, eye: &InFluid, render_distance: f32) -> FluidView {
    let fixed = |f: &Fog| {
        let scale = if f.relative { render_distance } else { 1.0 };
        FluidView { color: f.color.map(|c| f32::from(c) / 255.0), start: f.start * scale, end: f.end * scale }
    };
    match (eye.fluid, rule) {
        (Fluid::Lava, FluidFog::Java) => fixed(&JAVA_LAVA),
        (Fluid::PowderSnow, FluidFog::Java) => fixed(&JAVA_POWDER_SNOW),
        (Fluid::Lava, FluidFog::Pack) => fixed(&fogs.lava),
        (Fluid::PowderSnow, FluidFog::Pack) => fixed(&fogs.powder_snow),
        (Fluid::Water, FluidFog::Java) => {
            let water = fixed(&fogs.water(eye.biome).fog);
            let vision = water_vision(eye.seconds);
            // FogRenderer.computeFogColor scales the colour towards a full brightest channel.
            let brightest = water.color.into_iter().fold(0.0, f32::max);
            let color = match water.color.contains(&0.0) {
                true => water.color,
                false => water.color.map(|c| lerp(c, c / brightest, vision)),
            };
            FluidView { color, end: water.end * vision.max(0.25), ..water }
        }
        (Fluid::Water, FluidFog::Pack) => {
            let water = fogs.water(eye.biome);
            let to = fixed(&water.fog);
            let Some(t) = water.transition else { return to };
            let (from, p) = (fixed(&t.init), t.progress(eye.seconds));
            FluidView { color: std::array::from_fn(|i| lerp(from.color[i], to.color[i], p)), start: lerp(from.start, to.start, p), end: lerp(from.end, to.end, p) }
        }
    }
}

/// Java's `LocalPlayer.getWaterVision`: 0.6 over the first 100 ticks underwater, the rest over the
/// next 500.
pub fn water_vision(seconds: f32) -> f32 {
    let ticks = seconds * 20.0;
    0.6 * (ticks / 100.0).clamp(0.0, 1.0) + 0.4 * ((ticks - 100.0) / 500.0).clamp(0.0, 1.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water(seconds: f32) -> InFluid {
        InFluid { fluid: Fluid::Water, biome: None, seconds }
    }

    #[test]
    fn java_water_fog_reaches_further_and_brightens_underwater() {
        let fogs = FluidFogs::default();
        let (first, later, full) = [0.0, 5.0, 30.0].map(|s| view(FluidFog::Java, &fogs, &water(s), 160.0)).into();
        // The pack's default water (0..60) stands in for a biome's: a quarter of the reach at first.
        assert_eq!((first.end, later.end, full.end), (15.0, 36.0, 60.0));
        assert_eq!(full.color[2], 1.0);
        assert!(first.color[2] < 1.0 && later.color[2] > first.color[2]);
    }

    #[test]
    fn pack_water_fog_closes_in_from_its_initial_fog() {
        let fogs = FluidFogs::default();
        let at = |s| view(FluidFog::Pack, &fogs, &water(s), 160.0).end;
        assert!((at(0.0) - (0.01 + 0.25 * 59.99)).abs() < 1e-4);
        assert_eq!(at(30.0), 60.0);
    }

    #[test]
    fn lava_is_dense_in_both_looks() {
        let lava = InFluid { fluid: Fluid::Lava, biome: None, seconds: 0.0 };
        let (java, pack) = (view(FluidFog::Java, &FluidFogs::default(), &lava, 160.0), view(FluidFog::Pack, &FluidFogs::default(), &lava, 160.0));
        assert_eq!((java.start, java.end, pack.end), (0.25, 1.0, 0.64));
        assert_eq!(java.color, pack.color);
    }

    #[test]
    fn water_vision_matches_java_at_its_steps() {
        assert_eq!((water_vision(0.0), water_vision(5.0), water_vision(30.0), water_vision(60.0)), (0.0, 0.6, 1.0, 1.0));
    }
}
