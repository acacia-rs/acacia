//! Bit-exact float routines: chewxy/math32 `Sin`/`Cos` (bedsim), mgl32 vector helpers and the Minecraft sine
//! table as Linux BDS builds it.

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

const SIN32: [f32; 6] = [1.589623e-10, -2.5050747e-8, 2.7557314e-6, -0.0001984127, 0.008333334, -0.16666667];
const COS32: [f32; 6] = [-1.13585365e-11, 2.0875701e-9, -2.7557314e-7, 2.4801588e-5, -0.0013888889, 0.041666668];

/// Octant reduction shared by the math32 `Sin`/`Cos` ports.
#[allow(clippy::approx_constant)] // Cephes splits pi/4 into three parts; PI4A is deliberately short.
fn reduce32(x: f32) -> (u64, f32) {
    const PI4A: f32 = 0.7853981;
    const PI4B: f32 = 3.7748947e-8;
    const PI4C: f32 = 2.6951515e-15;
    let mut j = (x * 1.2732395_f32) as u64;
    let mut y = j as f32;
    if j & 1 == 1 {
        j += 1;
        y += 1.0;
    }
    j &= 7;
    (j, ((x - y * PI4A) - y * PI4B) - y * PI4C)
}

fn poly_sin32(z: f32, zz: f32) -> f32 {
    let s = &SIN32;
    z + z * zz * (((((s[0] * zz + s[1]) * zz + s[2]) * zz + s[3]) * zz + s[4]) * zz + s[5])
}

fn poly_cos32(zz: f32) -> f32 {
    let c = &COS32;
    1.0 - 0.5 * zz + zz * zz * (((((c[0] * zz + c[1]) * zz + c[2]) * zz + c[3]) * zz + c[4]) * zz + c[5])
}

/// chewxy/math32 `Sin` (float32 Cephes); falls back to libm beyond its fast range.
pub fn sin32(x: f32) -> f32 {
    if x == 0.0 || !x.is_finite() || x.abs() >= (1u32 << 29) as f32 {
        return x.sin();
    }
    let sign = x < 0.0;
    let (mut j, z) = reduce32(x.abs());
    let mut sign = sign;
    if j > 3 {
        sign = !sign;
        j -= 4;
    }
    let zz = z * z;
    let r = if j == 1 || j == 2 { poly_cos32(zz) } else { poly_sin32(z, zz) };
    if sign { -r } else { r }
}

/// chewxy/math32 `Cos` (float32 Cephes); falls back to libm beyond its fast range.
pub fn cos32(x: f32) -> f32 {
    if !x.is_finite() || x.abs() >= (1u32 << 29) as f32 {
        return x.cos();
    }
    let (mut j, z) = reduce32(x.abs());
    let mut sign = false;
    if j > 3 {
        j -= 4;
        sign = !sign;
    }
    if j > 1 {
        sign = !sign;
    }
    let zz = z * z;
    let r = if j == 1 || j == 2 { poly_sin32(z, zz) } else { poly_cos32(zz) };
    if sign { -r } else { r }
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
