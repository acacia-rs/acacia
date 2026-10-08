//! The item in the player's hand in first person, carried by the camera: Java's
//! `ItemInHandRenderer` arm offset, then the item models' `firstperson_righthand` display
//! (generated items: rotation 0, -90, 25, translation 1.13, 3.2, 1.13 sixteenths, scale 0.68;
//! blocks: rotation 0, 45, 0, scale 0.4).

use glam::{EulerRot, Mat3, Mat4, Vec3};

use super::ItemModel;
use crate::Camera;
use crate::entity::{EntityInstance, Pose};

/// Right hand, at rest, in view space (x right, y up, z towards the viewer).
const ARM: Vec3 = Vec3::new(0.56, -0.52, -0.72);

/// An item in another entity's right hand: `body` is the entity's model space to camera-relative
/// world space (as the entity pass places it), `hand` from [`crate::entity::bake::Mesh::right_hand`].
/// Java's `ItemInHandLayer` chain, run in Java's y-down model space (`FLIP_Y` converts at the
/// shoulder: Bedrock geometry is Java's with y mirrored): out of the arm (-90° x, 180° y), to the
/// fist (1, 2, -10 px), then the model's `thirdperson_righthand` display. Non-block items take the
/// `handheld` display (0, -90, 55; 0, 4, 0.5 px; 0.85), which mobs mostly hold; blocks 75, 45, 0;
/// 0, 2.5, 0 px; 0.375.
pub fn third_person(model: &ItemModel, body: Mat4, hand: Mat4, light_at: glam::DVec3) -> EntityInstance {
    let frame = body * held_frame(model.block, hand);
    EntityInstance { layers: model.layers.clone(), skin: Some(model.skin.clone()), position: light_at, yaw: 0.0, scale: 1.0, pose: Pose::default(), frame: Some(frame) }
}

/// [`third_person`]'s item mesh to the holder's model space.
pub fn held_frame(block: bool, hand: Mat4) -> Mat4 {
    const FLIP_Y: Vec3 = Vec3::new(1.0, -1.0, 1.0);
    let (rotation, shift, scale) =
        if block { (Vec3::new(75.0, 45.0, 0.0), Vec3::new(0.0, 2.5, 0.0), 0.375) } else { (Vec3::new(0.0, -90.0, 55.0), Vec3::new(0.0, 4.0, 0.5), 0.85) };
    let [rx, ry, rz] = rotation.to_array().map(f32::to_radians);
    hand * Mat4::from_scale(FLIP_Y)
        * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_translation((Vec3::new(1.0, 2.0, -10.0) + shift) / 16.0)
        * Mat4::from_euler(EulerRot::XYZ, rx, ry, rz)
        * Mat4::from_scale(Vec3::new(scale, scale, -scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resting_arm_holds_a_sword_forward_at_the_fist() {
        // Shoulder of a humanoid's right arm, in model space (blocks).
        let hand = Mat4::from_translation(Vec3::new(-5.0, 22.0, 0.0) / 16.0);
        let frame = held_frame(false, hand);
        // A sword sprite: hilt bottom-left, tip top-right (item mesh space).
        let (hilt, tip) = (frame.transform_point3(Vec3::new(-0.5, -0.5, 0.0)), frame.transform_point3(Vec3::new(0.5, 0.5, 0.0)));
        let blade = (tip - hilt).normalize();
        // Model space faces -z: forward, a little down, no sideways lean (Java's 55° twist is about the blade).
        assert!(blade.z < -0.9 && blade.y < 0.0 && blade.x.abs() < 0.1, "{blade}");
        let fist = (hilt + tip) / 2.0;
        assert!((fist.x + 0.375).abs() < 0.15 && (0.6..0.95).contains(&fist.y), "{fist}");
    }
}

/// `swing` is 0 to 1 through an arm swing (0 at rest).
pub fn first_person(model: &ItemModel, camera: &Camera, swing: f32) -> EntityInstance {
    let forward = camera.forward();
    let right = camera.right();
    let up = right.cross(forward);
    let view = Mat4::from_mat3(Mat3::from_cols(right, up, -forward));
    let (rotation, translation, scale) = if model.block {
        (Vec3::new(0.0, 45.0, 0.0), Vec3::ZERO, 0.4)
    } else {
        (Vec3::new(0.0, -90.0, 25.0), Vec3::new(1.13, 3.2, 1.13) / 16.0, 0.68)
    };
    // Java's swing: the arm dips and turns in, by sin of the progress.
    let s = (swing.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
    let arm = ARM + Vec3::new(-0.4 * s, 0.2 * s, -0.2 * s);
    let [rx, ry, rz] = rotation.to_array().map(f32::to_radians);
    // Meshes are stored with z mirrored (model space faces -z): mirror back into Java's item space.
    let frame = view
        * Mat4::from_translation(arm)
        * Mat4::from_rotation_y(-s * 0.35)
        * Mat4::from_translation(translation)
        * Mat4::from_euler(EulerRot::XYZ, rx, ry, rz)
        * Mat4::from_scale(Vec3::new(scale, scale, -scale));
    EntityInstance {
        layers: model.layers.clone(),
        skin: Some(model.skin.clone()),
        position: camera.position,
        yaw: 0.0,
        scale,
        pose: Pose::default(),
        frame: Some(frame),
    }
}
