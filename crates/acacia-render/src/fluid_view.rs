//! The fog with the camera in water, lava or powder snow. A look with [`FluidFog::Pack`] takes
//! its fog files ([`view`]); Java's rules are [`crate::sky::fog`].

use crate::biome::fog::{Fog, FluidFogs};
use crate::sky::fog::FogType;

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

impl InFluid {
    pub fn fog_type(&self) -> FogType {
        match self.fluid {
            Fluid::Water => FogType::Water,
            Fluid::Lava => FogType::Lava,
            Fluid::PowderSnow => FogType::PowderSnow,
        }
    }

    /// `waterVisionTime`: ticks with the eye in water, up to 600.
    pub fn water_vision_ticks(&self) -> i32 {
        if self.fluid == Fluid::Water { ((self.seconds * 20.0) as i32).min(600) } else { 0 }
    }
}

/// The fog drawn instead of the sky's: sRGB colour, opaque at `end` blocks from the camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidView {
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
}

/// Java's `textures/misc/underwater.png`, when the look has it (Bedrock draws no overlay).
pub fn load_underwater(files: &std::path::Path) -> Option<image::RgbaImage> {
    Some(image::open(crate::assets::image_file(files, "textures/misc/underwater")?).ok()?.into_rgba8())
}

/// The pack's fog in `eye`'s fluid; `render_distance` is in blocks, for fogs given as a share of it.
pub fn view(fogs: &FluidFogs, eye: &InFluid, render_distance: f32) -> FluidView {
    let fixed = |f: &Fog| {
        let scale = if f.relative { render_distance } else { 1.0 };
        FluidView { color: f.color.map(|c| f32::from(c) / 255.0), start: f.start * scale, end: f.end * scale }
    };
    match eye.fluid {
        Fluid::Lava => fixed(&fogs.lava),
        Fluid::PowderSnow => fixed(&fogs.powder_snow),
        Fluid::Water => {
            let water = fogs.water(eye.biome);
            let to = fixed(&water.fog);
            let Some(t) = water.transition else { return to };
            let (from, p) = (fixed(&t.init), t.progress(eye.seconds));
            FluidView { color: std::array::from_fn(|i| lerp(from.color[i], to.color[i], p)), start: lerp(from.start, to.start, p), end: lerp(from.end, to.end, p) }
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_water_fog_closes_in_from_its_initial_fog() {
        let fogs = FluidFogs::default();
        let at = |s| view(&fogs, &InFluid { fluid: Fluid::Water, biome: None, seconds: s }, 160.0).end;
        assert!((at(0.0) - (0.01 + 0.25 * 59.99)).abs() < 1e-4);
        assert_eq!(at(30.0), 60.0);
    }

    #[test]
    fn pack_lava_is_dense_and_vision_counts_ticks_in_water() {
        let lava = InFluid { fluid: Fluid::Lava, biome: None, seconds: 3.0 };
        assert_eq!(view(&FluidFogs::default(), &lava, 160.0).end, 0.64);
        assert_eq!((lava.water_vision_ticks(), lava.fog_type()), (0, FogType::Lava));
        let water = InFluid { fluid: Fluid::Water, seconds: 5.0, ..lava };
        assert_eq!((water.water_vision_ticks(), InFluid { seconds: 99.0, ..water }.water_vision_ticks()), (100, 600));
    }
}
