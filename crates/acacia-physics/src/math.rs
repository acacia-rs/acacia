//! Bit-exact float routines: mgl32 vector helpers and the Minecraft sine table as Linux BDS builds it.

use std::sync::OnceLock;

pub type Vec3 = [f32; 3];
pub type BlockPos = [i32; 3];

pub(crate) const PI32: f32 = std::f32::consts::PI;

static SIN_TABLE: OnceLock<Box<[f32]>> = OnceLock::new();

/// BDS fills its table with `sinf(i / 10430.378f)` (dumped from a live Linux BDS 1.26.52.3: all 65536 entries;
/// two thirds differ by an ulp from a correctly rounded sine, which moved every off-axis sprint jump).
fn sin_table() -> &'static [f32] {
    SIN_TABLE.get_or_init(|| (0..65536).map(|i| crate::sinf::sinf(i as f32 / 10430.378)).collect())
}

/// Minecraft's table sine.
pub fn mc_sin(v: f32) -> f32 {
    // Go's float32 -> uint16 conversion truncates through a signed 32-bit integer on amd64.
    sin_table()[((v * 10430.378) as i32 as u16) as usize]
}

/// Minecraft's table cosine.
pub fn mc_cos(v: f32) -> f32 {
    sin_table()[((v * 10430.378 + 16384.0) as i32 as u16) as usize]
}

/// BDS's wrap of a received yaw, in its f32 order: the `+ 180` rounds onto a grid two ulps coarse near ±53°, which
/// can move the angle across a sine-table step (strict BDS: predicts every 2026-10-28 cruise correction in four
/// traced flights; `course-human-linux` trace mismatches 8 → 6). Pitch is not wrapped (wrapping it adds corrections).
pub fn wrap_degrees(a: f32) -> f32 {
    let w = (a + 180.0) % 360.0;
    (if w < 0.0 { w + 360.0 } else { w }) + -180.0
}

pub fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min { min } else { v.min(max) }
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, s: f32) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn len_sqr(a: Vec3) -> f32 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

pub fn len(a: Vec3) -> f32 {
    len_sqr(a).sqrt()
}

pub fn hz_dist_sqr(a: Vec3) -> f32 {
    a[0] * a[0] + a[2] * a[2]
}

/// `v.floor() as i32`, NaN and saturation included; baseline x86-64 compiles `f32::floor` to a libm call.
#[inline]
pub fn floor_i32(v: f32) -> i32 {
    let t = v as i32;
    t.saturating_sub((v < t as f32) as i32)
}

/// `v.ceil() as i32`, as [`floor_i32`].
#[inline]
pub fn ceil_i32(v: f32) -> i32 {
    let t = v as i32;
    t.saturating_add((v > t as f32) as i32)
}

pub fn block_pos(v: Vec3) -> BlockPos {
    v.map(floor_i32)
}

pub fn pos_vec(p: BlockPos) -> Vec3 {
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

#[cfg(test)]
mod tests {
    use super::{ceil_i32, floor_i32};

    #[test]
    fn floor_and_ceil_i32_match_std() {
        let edges = [0.0, -0.0, 0.5, -0.5, 1.0, -1.0, -1e-7, 63.999_996, -64.000_01, 3e9, -3e9, 2_147_483_648.0, -2_147_483_904.0, f32::INFINITY, f32::NEG_INFINITY, f32::NAN];
        let sweep = (-20_000..20_000).map(|i| i as f32 * 0.003_7 - 0.25);
        for v in edges.into_iter().chain(sweep) {
            assert_eq!(floor_i32(v), v.floor() as i32, "floor {v}");
            assert_eq!(ceil_i32(v), v.ceil() as i32, "ceil {v}");
        }
    }
}
