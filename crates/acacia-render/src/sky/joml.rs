//! Rotations as the game's `PoseStack` makes them (`Axis.rotate` → JOML `Matrix4f.rotateX/Y/Z`):
//! the float sine, and the cosine as `sqrt(1 - sin²)`, which near 90 degrees is off from glam's
//! `from_rotation_*` by ~1e-4 relative.

use glam::{Mat4, Vec4};

/// `org.joml.Math.cosFromSin` (no fast math).
fn cos_from_sin(sin: f32, angle: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};
    let cos = (1.0 - sin * sin).sqrt();
    let a = angle + FRAC_PI_2;
    let mut b = a - (a / TAU) as i32 as f32 * TAU;
    if f64::from(b) < 0.0 {
        b += TAU;
    }
    if b >= PI { -cos } else { cos }
}

fn sin_cos(angle: f32) -> (f32, f32) {
    let sin = f64::from(angle).sin() as f32;
    (sin, cos_from_sin(sin, angle))
}

pub fn rotation_x(radians: f32) -> Mat4 {
    let (s, c) = sin_cos(radians);
    Mat4::from_cols(Vec4::X, Vec4::new(0.0, c, s, 0.0), Vec4::new(0.0, -s, c, 0.0), Vec4::W)
}

pub fn rotation_y(radians: f32) -> Mat4 {
    let (s, c) = sin_cos(radians);
    Mat4::from_cols(Vec4::new(c, 0.0, -s, 0.0), Vec4::Y, Vec4::new(s, 0.0, c, 0.0), Vec4::W)
}

pub fn rotation_z(radians: f32) -> Mat4 {
    let (s, c) = sin_cos(radians);
    Mat4::from_cols(Vec4::new(c, s, 0.0, 0.0), Vec4::new(-s, c, 0.0, 0.0), Vec4::Z, Vec4::W)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_glam_but_for_the_cosine() {
        for degrees in [0.0f32, 30.0, 89.5, 90.0, 180.0, 200.0, 270.0, -90.0] {
            let a = degrees.to_radians();
            assert!(rotation_x(a).abs_diff_eq(Mat4::from_rotation_x(a), 2e-4), "{degrees}");
            assert!(rotation_y(a).abs_diff_eq(Mat4::from_rotation_y(a), 2e-4), "{degrees}");
            assert!(rotation_z(a).abs_diff_eq(Mat4::from_rotation_z(a), 2e-4), "{degrees}");
        }
        // The game's sun frame at 89.5 degrees: 100 * cos lands at 0.8727814, not 0.8726562.
        let m = rotation_x(89.5 * (std::f64::consts::PI / 180.0) as f32);
        assert!((m.y_axis.y * 100.0 - 0.872_781_4).abs() < 1e-5, "{}", m.y_axis.y * 100.0);
    }
}
