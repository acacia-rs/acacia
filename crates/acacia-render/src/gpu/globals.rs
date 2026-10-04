//! The uniform block every shader reads (`globals.wgsl`).

use glam::{IVec3, Mat4, Vec3};

use crate::look::Look;

/// Brightness of unlit blocks, with and without sky light (Java's nether ambient is 0.1).
const AMBIENT: (f32, f32) = (0.02, 0.1);

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
    /// x: ambient brightness, y: [`crate::sky::Sky::darken`].
    light: [f32; 4],
}

impl Globals {
    /// `fog_color` is linear rgb; fog turns opaque at `fog_end` blocks.
    pub fn new(view_proj: Mat4, (block, frac): (IVec3, Vec3), fog_color: [f32; 3], fog_end: f32, look: &Look, has_sky: bool, darken: f32) -> Globals {
        let flag = |on: bool| f32::from(u8::from(on));
        Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_block: [block.x, block.y, block.z, 0],
            cam_frac: [frac.x, frac.y, frac.z, 0.0],
            water: [look.water_alpha.unwrap_or(-1.0), 0.0, 0.0, 0.0],
            fog: [fog_color[0], fog_color[1], fog_color[2], fog_end],
            fog_shape: [look.fog.start(fog_end), flag(look.fog.cylinder), flag(look.fog.linear), 0.0],
            light: [if has_sky { AMBIENT.0 } else { AMBIENT.1 }, darken, 0.0, 0.0],
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
    const MODULES: [(&str, &str); 3] = [
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
