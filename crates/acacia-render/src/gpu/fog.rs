//! The fog a frame draws with: the sky's colour and the look's haze, or the fluid the camera is in.

use super::Renderer;
use super::globals::srgb_to_linear;
use crate::fluid_view::{self, Fluid};

pub(super) struct FrameFog {
    /// Linear rgb: the fog's, and the clear colour.
    pub color: [f32; 3],
    /// Where the second, spherical fog starts and is opaque ([`super::globals::Globals::new`]).
    pub haze: Option<(f32, f32)>,
    /// The camera is in a fluid: no sky, sun or clouds.
    pub in_fluid: bool,
    pub underwater: bool,
}

impl Renderer {
    /// `sky_color` is sRGB.
    pub(super) fn frame_fog(&self, sky_color: [f32; 3]) -> FrameFog {
        if let Some(eye) = &self.in_fluid {
            // The fluid's fog stands in for the look's haze, so it is linear and spherical in both looks.
            let v = fluid_view::view(self.look.fluid_fog, self.biomes.fluid_fogs(), eye, self.fog_distance);
            return FrameFog { color: srgb_to_linear(v.color), haze: Some((v.start, v.end)), in_fluid: true, underwater: eye.fluid == Fluid::Water };
        }
        // Only the Nether is that low; the End hazes as the overworld does.
        let nether = self.world().is_some_and(|w| !w.dimension().sky && w.dimension().height <= 128);
        let haze = self.look.fog.haze.map(|h| if nether { h.nether } else { h.overworld });
        FrameFog { color: srgb_to_linear(sky_color), haze, in_fluid: false, underwater: false }
    }
}
