//! The player's own right arm in first person, shown while the main hand is empty: Java's
//! `ItemInHandRenderer.renderPlayerArm`, drawing the player model with every other bone hidden.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3};

use crate::Camera;
use crate::entity::{EntityInstance, Layer, Pose, Skin};

/// The bones of the arm and its sleeve, as meshes name them (lowercase).
const ARM: [&str; 2] = ["rightarm", "rightsleeve"];
/// A humanoid's neck, where Java's model space has its origin, in blocks over the feet.
const NECK: f32 = 1.5;

/// Java's model space (y down from the neck, 1/16 block) placed in view space; `swing` is 0 to
/// 1 through an arm swing.
fn placed(swing: f32) -> Mat4 {
    let root = swing.sqrt();
    let at = Vec3::new(-0.3 * (root * PI).sin() + 0.640_000_05, 0.4 * (root * TAU).sin() - 0.6, -0.4 * (swing * PI).sin() - 0.719_999_97);
    Mat4::from_translation(at)
        * Mat4::from_rotation_y(45f32.to_radians())
        * Mat4::from_rotation_y(((root * PI).sin() * 70.0).to_radians())
        * Mat4::from_rotation_z(((swing * swing * PI).sin() * -20.0).to_radians())
        * Mat4::from_translation(Vec3::new(-1.0, 3.6, 3.5))
        * Mat4::from_rotation_z(120f32.to_radians())
        * Mat4::from_rotation_x(200f32.to_radians())
        * Mat4::from_rotation_y((-135f32).to_radians())
        * Mat4::from_translation(Vec3::new(5.6, 0.0, 0.0))
}

/// The pack's model space (y up from the feet) to Java's.
fn to_java() -> Mat4 {
    Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)) * Mat4::from_translation(Vec3::new(0.0, -NECK, 0.0))
}

/// `layers` and `skin` are the own player's; `bones` its mesh's, in order.
pub fn first_person(layers: &[Layer], skin: Option<Arc<Skin>>, bones: &[String], camera: &Camera, swing: f32) -> EntityInstance {
    let mut hidden = [0u32; 4];
    for (index, _) in bones.iter().enumerate().take(128).filter(|(_, bone)| !ARM.contains(&bone.as_str())) {
        hidden[index / 32] |= 1 << (index % 32);
    }
    let shown = |layer: &Layer| Layer { hidden: std::array::from_fn(|word| layer.hidden[word] | hidden[word]), ..layer.clone() };
    let (forward, right) = (camera.forward(), camera.right());
    let view = Mat4::from_mat3(Mat3::from_cols(right, right.cross(forward), -forward));
    EntityInstance {
        layers: layers.iter().map(shown).collect(),
        skin,
        position: camera.position,
        yaw: 0.0,
        scale: 1.0,
        pose: Pose::default(),
        frame: Some(view * placed(swing.clamp(0.0, 1.0)) * to_java()),
        hurt: false,
        glint: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_resting_arm_reaches_into_the_view_from_the_lower_right() {
        // The arm's box in the pack's space: 4 px wide from the shoulder at 22 px down to 12.
        let at = |x: f32, y: f32| (placed(0.0) * to_java()).transform_point3(Vec3::new(x, y, 0.0) / 16.0);
        let (shoulder, fist) = (at(-6.0, 23.0), at(-6.0, 12.0));
        // The fist is further in than the shoulder and, on the screen, nearer the middle.
        assert!(fist.z < -0.3 && fist.z < shoulder.z, "{shoulder} {fist}");
        let on_screen = |p: Vec3| (p.x / -p.z, p.y / -p.z);
        let ((sx, sy), (fx, fy)) = (on_screen(shoulder), on_screen(fist));
        assert!(fx > 0.0 && fx < sx && fy < 0.0 && fy > sy, "{shoulder} {fist}");
    }
}
