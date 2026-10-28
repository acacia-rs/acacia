//! The uniform block every shader reads (`globals.wgsl`).

use glam::{IVec3, Mat4, Vec3};

use crate::light::lightmap::{self, Table};
use crate::look::{Lighting, Look};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_block: [i32; 4],
    cam_frac: [f32; 4],
    /// x: [`Look::water_alpha`], negative for the texture's alpha.
    water: [f32; 4],
    fog: [f32; 4],
    /// x: distance where fog begins, y: cylinder, z: linear ([`crate::look::Fog`]).
    fog_shape: [f32; 4],
    /// x, y: the [`crate::look::Haze`] of the dimension, zero for none.
    haze: [f32; 4],
    /// [`Look::ambient_occlusion`].
    ao: [f32; 4],
    /// x: 1 for [`Lighting::Java`], y: 1 for Java's `ENTITY_IN_UI` lights ([`Globals::in_pictures`]).
    light: [f32; 4],
    /// [`lightmap::packed`].
    lightmap: [[u32; 4]; 64],
}

impl Globals {
    /// `fog_color` is linear rgb; fog turns opaque at `fog_end` blocks. `haze` is where the look's
    /// haze starts and ends here.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        view_proj: Mat4,
        (block, frac): (IVec3, Vec3),
        fog_color: [f32; 3],
        fog_end: f32,
        look: &Look,
        haze: Option<(f32, f32)>,
        lightmap: &Table,
    ) -> Globals {
        let flag = |on: bool| f32::from(u8::from(on));
        let (haze_start, haze_end) = haze.unwrap_or_default();
        Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_block: [block.x, block.y, block.z, 0],
            cam_frac: [frac.x, frac.y, frac.z, 0.0],
            water: [look.water_alpha.unwrap_or(-1.0), 0.0, 0.0, 0.0],
            fog: [fog_color[0], fog_color[1], fog_color[2], fog_end],
            fog_shape: [look.fog.start(fog_end), flag(look.fog.cylinder), flag(look.fog.linear), 0.0],
            haze: [haze_start, haze_end, 0.0, 0.0],
            ao: look.ambient_occlusion,
            light: [flag(look.lighting == Lighting::Java), 0.0, 0.0, 0.0],
            lightmap: lightmap::packed(lightmap),
        }
    }

    /// These globals for pictures-in-picture drawn into a `window`-sized texture: positions in
    /// window pixels (y down, z towards the viewer, ±1000 deep as `PictureInPictureRenderer`'s
    /// ortho), no fog, the inventory's lights.
    pub(super) fn in_pictures(self, [width, height]: [u32; 2]) -> Globals {
        let (w, h) = (width as f32, height as f32);
        let ortho = Mat4::from_cols_array(&[2.0 / w, 0.0, 0.0, 0.0, 0.0, -2.0 / h, 0.0, 0.0, 0.0, 0.0, 1.0 / 2000.0, 0.0, -1.0, 1.0, 0.5, 1.0]);
        Globals {
            view_proj: ortho.to_cols_array_2d(),
            cam_block: [0; 4],
            cam_frac: [0.0; 4],
            fog: [0.0, 0.0, 0.0, f32::MAX],
            fog_shape: [f32::MAX / 2.0, 0.0, 1.0, 0.0],
            haze: [0.0; 4],
            light: [self.light[0], 1.0, 0.0, 0.0],
            ..self
        }
    }
}

pub(super) fn srgb_to_linear(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) })
}

#[cfg(test)]
mod tests {
    use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
    use wgpu::naga::{TypeInner, front::wgsl};

    use super::Globals;

    /// Every shader module, as the pipelines assemble them.
    const MODULES: [(&str, &str); 8] = [
        ("weather", concat!(include_str!("globals.wgsl"), include_str!("weather.wgsl"))),
        ("bolts", concat!(include_str!("globals.wgsl"), include_str!("bolts.wgsl"))),
        ("clouds", concat!(include_str!("globals.wgsl"), include_str!("clouds.wgsl"))),
        ("sign text", concat!(include_str!("globals.wgsl"), include_str!("sign_text.wgsl"))),
        ("screen effect", concat!(include_str!("globals.wgsl"), include_str!("screen_effect.wgsl"))),
        ("terrain", concat!(include_str!("globals.wgsl"), include_str!("terrain.wgsl"), include_str!("light.wgsl"), include_str!("model.wgsl"))),
        ("entity", concat!(include_str!("globals.wgsl"), include_str!("entity.wgsl"))),
        ("sky", concat!(include_str!("globals.wgsl"), include_str!("sky.wgsl"))),
    ];

    #[test]
    fn shaders_validate_and_agree_on_the_uniform_size() {
        for (name, source) in MODULES {
            let module = wgsl::parse_str(source).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
            Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            let span = module.types.iter().find_map(|(_, t)| match t.inner {
                TypeInner::Struct { span, .. } if t.name.as_deref() == Some("Globals") => Some(span),
                _ => None,
            });
            assert_eq!(span, Some(size_of::<Globals>() as u32), "{name}");
        }
    }
}
