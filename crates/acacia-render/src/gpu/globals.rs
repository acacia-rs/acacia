//! The uniform block every shader reads (`globals.wgsl`).

use glam::{IVec3, Mat4, Vec3};

use crate::blocks::tint::WATER_ALPHA;

/// Brightness of unlit blocks, with and without sky light (Java's nether ambient is 0.1).
const AMBIENT: (f32, f32) = (0.02, 0.1);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_block: [i32; 4],
    cam_frac: [f32; 4],
    water: [f32; 4],
    fog: [f32; 4],
    /// x: ambient brightness, y: [`crate::sky::Sky::darken`].
    light: [f32; 4],
}

impl Globals {
    /// `fog` is linear rgb and the distance where it turns opaque.
    pub fn new(view_proj: Mat4, (block, frac): (IVec3, Vec3), fog: [f32; 4], has_sky: bool, darken: f32) -> Globals {
        Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_block: [block.x, block.y, block.z, 0],
            cam_frac: [frac.x, frac.y, frac.z, 0.0],
            water: [WATER_ALPHA, 0.0, 0.0, 0.0],
            fog,
            light: [if has_sky { AMBIENT.0 } else { AMBIENT.1 }, darken, 0.0, 0.0],
        }
    }
}

pub(super) fn srgb_to_linear(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) })
}
