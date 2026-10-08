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
