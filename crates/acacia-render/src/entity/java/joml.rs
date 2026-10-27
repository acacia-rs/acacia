//! JOML's float quaternion and 3×3 rotation arithmetic, in its own operation order, for
//! `ModelPart.rotateBy`: rotation matrix from the part's angles (`Matrix3f.rotationZYX`), times
//! the quaternion's matrix (`Matrix3f.rotate`), back to angles (`getEulerAnglesZYX`).

use super::model::PartPose;
use super::mth::joml_sin_cos;

/// `Quaternionf`: (x, y, z, w), the identity to start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat(pub [f32; 4]);

impl Default for Quat {
    fn default() -> Quat {
        Quat([0.0, 0.0, 0.0, 1.0])
    }
}

impl Quat {
    fn half(angle: f32) -> (f32, f32) {
        joml_sin_cos(angle * 0.5)
    }

    pub fn rotate_x(self, angle: f32) -> Quat {
        let (s, c) = Quat::half(angle);
        let [x, y, z, w] = self.0;
        Quat([w * s + x * c, y * c + z * s, z * c - y * s, w * c - x * s])
    }

    pub fn rotate_y(self, angle: f32) -> Quat {
        let (s, c) = Quat::half(angle);
        let [x, y, z, w] = self.0;
        Quat([x * c - z * s, w * s + y * c, x * s + z * c, w * c - y * s])
    }

    pub fn rotate_z(self, angle: f32) -> Quat {
        let (s, c) = Quat::half(angle);
        let [x, y, z, w] = self.0;
        Quat([x * c + y * s, y * c - x * s, w * s + z * c, w * c - z * s])
    }
}

/// `Quaternionf.rotationZYX`, as `ModelPart.translateAndRotate` turns a part: half angles through
/// `cosFromSin`, which loses digits near ±90°; a long part turned near ±180° shows it.
pub fn rotation_zyx_quat(z: f32, y: f32, x: f32) -> glam::Quat {
    let (sx, cx) = joml_sin_cos(x * 0.5);
    let (sy, cy) = joml_sin_cos(y * 0.5);
    let (sz, cz) = joml_sin_cos(z * 0.5);
    let (cycz, sysz, sycz, cysz) = (cy * cz, sy * sz, sy * cz, cy * sz);
    glam::Quat::from_xyzw(sx * cycz - cx * sysz, cx * sycz + sx * cysz, cx * cysz - sx * sycz, cx * cycz + sx * sysz)
}

/// Row-major as JOML names them: `m[col][row]` is JOML's `m<col><row>`.
type M3 = [[f32; 3]; 3];

fn rotation_zyx(z: f32, y: f32, x: f32) -> M3 {
    let (sx, cx) = joml_sin_cos(x);
    let (sy, cy) = joml_sin_cos(y);
    let (sz, cz) = joml_sin_cos(z);
    let (nm00, nm01, nm10, nm11) = (cz, sz, -sz, cz);
    let (nm20, nm21, nm22) = (nm00 * sy, nm01 * sy, cy);
    [
        [nm00 * cy, nm01 * cy, -sy],
        [nm10 * cx + nm20 * sx, nm11 * cx + nm21 * sx, nm22 * sx],
        [nm10 * -sx + nm20 * cx, nm11 * -sx + nm21 * cx, nm22 * cx],
    ]
}

fn rotate(m: M3, q: Quat) -> M3 {
    let [x, y, z, w] = q.0;
    let (w2, x2, y2, z2) = (w * w, x * x, y * y, z * z);
    let twice = |a: f32| a + a;
    let (dzw, dxy, dxz, dyw, dyz, dxw) = (twice(z * w), twice(x * y), twice(x * z), twice(y * w), twice(y * z), twice(x * w));
    let r = [
        [w2 + x2 - z2 - y2, dxy + dzw, dxz - dyw],
        [dxy - dzw, y2 - z2 + w2 - x2, dyz + dxw],
        [dyw + dxz, dyz - dxw, z2 - y2 - x2 + w2],
    ];
    std::array::from_fn(|c| std::array::from_fn(|row| m[0][row] * r[c][0] + m[1][row] * r[c][1] + m[2][row] * r[c][2]))
}

/// `ModelPart.rotateBy`.
pub fn rotate_by(pose: &mut PartPose, q: Quat) {
    let m = rotate(rotation_zyx(pose.z_rot, pose.y_rot, pose.x_rot), q);
    pose.x_rot = f64::from(m[1][2]).atan2(f64::from(m[2][2])) as f32;
    pose.y_rot = f64::from(-m[0][2]).atan2(f64::from(1.0 - m[0][2] * m[0][2]).sqrt() as f32 as f64) as f32;
    pose.z_rot = f64::from(m[0][1]).atan2(f64::from(m[0][0])) as f32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_about_x_adds_to_x_rot() {
        let mut pose = PartPose { x_rot: 0.3, ..PartPose::default() };
        rotate_by(&mut pose, Quat::default().rotate_x(0.5));
        assert!((pose.x_rot - 0.8).abs() < 1e-6 && pose.y_rot.abs() < 1e-6 && pose.z_rot.abs() < 1e-6, "{pose:?}");
    }
}
