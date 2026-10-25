//! Rotations as the game's `PoseStack` makes them (`Axis.rotation` then `Matrix4f.rotate`): JOML
//! takes the half angle's float sine, its cosine from that sine, and squares them into the
//! matrix, which near 90 degrees is off from glam's `from_rotation_*` by ~1e-5 per unit.

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

/// `Quaternionf.rotationX/Y/Z(angle)` as (x, y, z, w); `axis` 0, 1 or 2.
fn quaternion(axis: usize, angle: f32) -> [f32; 4] {
    let sin = (f64::from(angle * 0.5)).sin() as f32;
    let mut q = [0.0, 0.0, 0.0, cos_from_sin(sin, angle * 0.5)];
    q[axis] = sin;
    q
}

/// `Matrix4f.rotation(Quaternionfc)`.
fn matrix([x, y, z, w]: [f32; 4]) -> Mat4 {
    let (w2, x2, y2, z2) = (w * w, x * x, y * y, z * z);
    let (zw, xy, xz, yw, yz, xw) = (z * w, x * y, x * z, y * w, y * z, x * w);
    let (dzw, dxy, dxz, dyw, dyz, dxw) = (zw + zw, xy + xy, xz + xz, yw + yw, yz + yz, xw + xw);
    Mat4::from_cols(
        Vec4::new(w2 + x2 - z2 - y2, dxy + dzw, dxz - dyw, 0.0),
        Vec4::new(-dzw + dxy, y2 - z2 + w2 - x2, dyz + dxw, 0.0),
        Vec4::new(dyw + dxz, dyz - dxw, z2 - y2 - x2 + w2, 0.0),
        Vec4::W,
    )
}

pub fn rotation_x(radians: f32) -> Mat4 {
    matrix(quaternion(0, radians))
}

pub fn rotation_y(radians: f32) -> Mat4 {
    matrix(quaternion(1, radians))
}

pub fn rotation_z(radians: f32) -> Mat4 {
    matrix(quaternion(2, radians))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_glam_up_to_float_squaring() {
        for degrees in [0.0f32, 30.0, 89.5, 90.0, 180.0, 270.0, -90.0] {
            let a = degrees.to_radians();
            assert!(rotation_x(a).abs_diff_eq(Mat4::from_rotation_x(a), 1e-4), "{degrees}");
            assert!(rotation_y(a).abs_diff_eq(Mat4::from_rotation_y(a), 1e-4), "{degrees}");
            assert!(rotation_z(a).abs_diff_eq(Mat4::from_rotation_z(a), 1e-4), "{degrees}");
        }
    }
}
