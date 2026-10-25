//! `Mth.sin` / `Mth.cos`: Java's 65536-entry float sine table, indexed through a double-to-long
//! cast, as every model's `setupAnim` calls them (after Horizon's `geometry/trig.rs`, MIT).

use std::sync::OnceLock;

const SCALE: f64 = 10430.378350470453;

fn table() -> &'static [f32] {
    static SINE: OnceLock<Box<[f32]>> = OnceLock::new();
    SINE.get_or_init(|| (0..65536).map(|i| (f64::from(i) / SCALE).sin() as f32).collect())
}

pub fn sin(angle: f32) -> f32 {
    table()[((f64::from(angle) * SCALE) as i64 & 65535) as usize]
}

pub fn cos(angle: f32) -> f32 {
    table()[((f64::from(angle) * SCALE + 16384.0) as i64 & 65535) as usize]
}

/// `Mth.rotLerpRad`.
pub fn rot_lerp_rad(amount: f32, from: f32, to: f32) -> f32 {
    use std::f32::consts::PI;
    let mut diff = to - from;
    while diff < -PI {
        diff += PI * 2.0;
    }
    while diff >= PI {
        diff -= PI * 2.0;
    }
    from + amount * diff
}

/// `Mth.rotLerp`: wraps the difference as degrees, though models hand it radians.
pub fn rot_lerp(amount: f32, from: f32, to: f32) -> f32 {
    from + amount * wrap_degrees(to - from)
}

/// `Mth.wrapDegrees(float)`: into [-180, 180).
pub fn wrap_degrees(angle: f32) -> f32 {
    let mut angle = angle % 360.0;
    if angle >= 180.0 {
        angle -= 360.0;
    }
    if angle < -180.0 {
        angle += 360.0;
    }
    angle
}

/// Java's `(float) (Math.PI / d)`: divided in double, then rounded once.
pub fn pi_div(d: f64) -> f32 {
    (std::f64::consts::PI / d) as f32
}

pub fn lerp(amount: f32, from: f32, to: f32) -> f32 {
    from + amount * (to - from)
}

/// JOML's sine and cosine of a `Matrix4f.rotateX/Y/Z` angle: the cosine is `sqrt(1 - sin²)` in
/// floats, signed by quadrant (`Math.cosFromSin`), off by up to 1e-5 near ±90°.
pub fn joml_sin_cos(angle: f32) -> (f32, f32) {
    use std::f32::consts::PI;
    let sin = f64::from(angle).sin() as f32;
    let cos = f64::from(1.0 - sin * sin).sqrt() as f32;
    let a = angle + PI / 2.0;
    let mut b = a - (a / (PI * 2.0)) as i32 as f32 * (PI * 2.0);
    if b < 0.0 {
        b += PI * 2.0;
    }
    (sin, if b >= PI { -cos } else { cos })
}

/// `Axis.rotateDegrees` on a matrix: `axis` is ±X, ±Y or ±Z.
pub fn joml_rotation(axis: glam::Vec3, degrees: f32) -> glam::Mat4 {
    use glam::{Mat4, Vec4};
    let angle = degrees * (std::f64::consts::PI / 180.0) as f32 * (axis.x + axis.y + axis.z);
    let (s, c) = joml_sin_cos(angle);
    let (x, y, z) = (axis.x != 0.0, axis.y != 0.0, axis.z != 0.0);
    match (x, y, z) {
        (true, _, _) => Mat4::from_cols(Vec4::X, Vec4::new(0.0, c, s, 0.0), Vec4::new(0.0, -s, c, 0.0), Vec4::W),
        (_, true, _) => Mat4::from_cols(Vec4::new(c, 0.0, -s, 0.0), Vec4::Y, Vec4::new(s, 0.0, c, 0.0), Vec4::W),
        _ => Mat4::from_cols(Vec4::new(c, s, 0.0, 0.0), Vec4::new(-s, c, 0.0, 0.0), Vec4::Z, Vec4::W),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn table_lookup_quantises_and_wraps() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert!((sin(PI / 2.0) - 1.0).abs() < 1e-6);
        // Negative angles wrap through the mask, not a sign flip of the lookup.
        assert_eq!(sin(-PI / 2.0), table()[49152]);
    }

    #[test]
    fn rot_lerp_takes_the_short_way() {
        assert!((rot_lerp_rad(0.5, 3.0, -3.0) - (3.0 + (2.0 * PI - 6.0) / 2.0)).abs() < 1e-5);
    }
}
