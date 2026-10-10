//! Held items. In first person the camera carries them: Java's `ItemInHandRenderer` arm offset,
//! then the item model's `firstperson_*hand` display. On another entity they follow its arm
//! (`ItemInHandLayer`) with the `thirdperson_*hand` display. See [`Display`] for the models read.

use glam::{EulerRot, Mat3, Mat4, Vec3};

use super::{Form, ItemModel, shield};
use crate::Camera;
use crate::entity::{EntityInstance, Pose};
use crate::glint::Glint;

/// Right hand, at rest, in view space (x right, y up, z towards the viewer).
const ARM: Vec3 = Vec3::new(0.56, -0.52, -0.72);

/// An item in another entity's hand: `body` is the entity's model space to camera-relative
/// world space (as the entity pass places it), `hand` from [`crate::entity::bake::Mesh::hand`].
pub fn third_person(model: &ItemModel, body: Mat4, hand: Mat4, left: bool, light_at: glam::DVec3) -> EntityInstance {
    let frame = body * held_frame(Display::third_person(model.form, left), hand, left);
    let glint = model.glint.then_some(Glint::Item);
    EntityInstance { layers: model.layers.clone(), skin: Some(model.skin.clone()), position: light_at, yaw: 0.0, scale: 1.0, pose: Pose::default(), frame: Some(frame), hurt: false, glint }
}

/// [`third_person`]'s item mesh to the holder's model space: Java's `ItemInHandLayer` chain, run
/// in Java's y-down model space (`FLIP_Y` converts at the shoulder: Bedrock geometry is Java's
/// with y mirrored): out of the arm (-90° x, 180° y), to the fist (1, 2, -10 px; -1 in the left
/// hand), then the display.
pub fn held_frame(display: Display, hand: Mat4, left: bool) -> Mat4 {
    const FLIP_Y: Vec3 = Vec3::new(1.0, -1.0, 1.0);
    let side = if left { -1.0 } else { 1.0 };
    hand * Mat4::from_scale(FLIP_Y)
        * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_translation(Vec3::new(side, 2.0, -10.0) / 16.0)
        * display.frame()
        * Mat4::from_scale(MESH)
}

/// Meshes are stored with z mirrored (model space faces -z): this mirrors them back into Java's item space.
const MESH: Vec3 = Vec3::new(1.0, 1.0, -1.0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resting_arm_holds_a_sword_forward_at_the_fist() {
        // Shoulder of a humanoid's right arm, in model space (blocks).
        let hand = Mat4::from_translation(Vec3::new(-5.0, 22.0, 0.0) / 16.0);
        let frame = held_frame(Display::third_person(Form::Flat, false), hand, false);
        // A sword sprite: hilt bottom-left, tip top-right (item mesh space).
        let ends = |frame: Mat4| (frame.transform_point3(Vec3::new(-0.5, -0.5, 0.0)), frame.transform_point3(Vec3::new(0.5, 0.5, 0.0)));
        let (hilt, tip) = ends(frame);
        let blade = (tip - hilt).normalize();
        // Model space faces -z: forward, a little down, no sideways lean (Java's 55° twist is about the blade).
        assert!(blade.z < -0.9 && blade.y < 0.0 && blade.x.abs() < 0.1, "{blade}");
        let fist = (hilt + tip) / 2.0;
        assert!((fist.x + 0.375).abs() < 0.15 && (0.6..0.95).contains(&fist.y), "{fist}");
        // The left hand holds it pointing the same way, at the other fist.
        let (left_hilt, left_tip) = ends(held_frame(Display::third_person(Form::Flat, true), Mat4::from_translation(Vec3::new(5.0, 22.0, 0.0) / 16.0), true));
        assert!((left_tip - left_hilt).normalize().abs_diff_eq(blade, 1e-4), "{left_hilt} {left_tip}");
        let left_fist = (left_hilt + left_tip) / 2.0;
        assert!((left_fist.x + fist.x).abs() < 1e-4 && (left_fist.y - fist.y).abs() < 1e-4, "{left_fist} {fist}");
    }

    #[test]
    fn a_drunk_potion_stays_in_view_at_the_bottom_centre() {
        let display = Display::first_person("minecraft:potion", Form::Flat, false, false);
        // Half the view's height over its depth at Java's 70° hand field of view.
        let half = 35.0f32.to_radians().tan();
        for ticks in [8.0, 16.0, 22.0, 30.0] {
            let centre = (eat_transform(ticks, 32.0) * Mat4::from_translation(ARM) * display.frame()).transform_point3(Vec3::ZERO);
            let (x, y) = (centre.x / -centre.z / half, centre.y / -centre.z / half);
            assert!(centre.z < -0.3 && x.abs() < 0.4 && (-0.95..-0.3).contains(&y), "{ticks}: {centre} at {x} {y}");
        }
    }
}

/// The held item in use, for Java's first-person use animations; `ticks` since the use began.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Using {
    /// Food or drink taking `duration` ticks.
    Eat { ticks: f32, duration: f32 },
    Bow { ticks: f32 },
}

/// `applyEatTransform`, in view space before the arm's offset: the item rises to the mouth, turned
/// in, and bobs while eaten.
fn eat_transform(ticks: f32, duration: f32) -> Mat4 {
    let remaining = (duration - ticks).max(0.0) + 1.0;
    let g = remaining / duration;
    let bob = if g < 0.8 { ((remaining / 4.0 * std::f32::consts::PI).cos() * 0.1).abs() } else { 0.0 };
    let h = 1.0 - g.min(1.0).powi(27);
    Mat4::from_translation(Vec3::new(h * 0.6, bob - h * 0.5, 0.0))
        * Mat4::from_rotation_y((h * 90.0).to_radians())
        * Mat4::from_rotation_x((h * 10.0).to_radians())
        * Mat4::from_rotation_z((h * 30.0).to_radians())
}

/// The bow held drawn, after the arm's offset; Java's pull `(m² + 2m) / 3` of the seconds drawn.
fn bow_transform(ticks: f32) -> Mat4 {
    let m = ticks / 20.0;
    let pull = ((m * m + m * 2.0) / 3.0).min(1.0);
    let shake = if pull > 0.1 { ((ticks - 0.1) * 1.3).sin() * (pull - 0.1) * 0.004 } else { 0.0 };
    Mat4::from_translation(Vec3::new(-0.278_568_2, 0.183_443_87, 0.157_315_31))
        * Mat4::from_rotation_x((-13.935f32).to_radians())
        * Mat4::from_rotation_y(35.3f32.to_radians())
        * Mat4::from_rotation_z((-9.785f32).to_radians())
        * Mat4::from_translation(Vec3::new(0.0, shake, pull * 0.04))
        * Mat4::from_scale(Vec3::new(1.0, 1.0, 1.0 + pull * 0.2))
        * Mat4::from_rotation_y((-45.0f32).to_radians())
}

/// An item model's `display` entry as `ItemTransform.apply` uses it for one hand: rotation in
/// degrees, translation in blocks, scale. Java's `item/generated` and `item/handheld`,
/// `block/block`, `item/bow` and the shield's are built in; other models' own are not read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    rotation: Vec3,
    translation: Vec3,
    scale: f32,
}

impl Display {
    /// An entry as the model files write it: the translation in 1/16 block.
    pub(super) fn px(rotation: [f32; 3], translation: [f32; 3], scale: f32) -> Display {
        Display { rotation: rotation.into(), translation: Vec3::from(translation) / 16.0, scale }
    }

    /// A left hand's entry as `ItemTransform.apply` reads it: mirrored in x.
    fn mirrored(self) -> Display {
        Display { rotation: self.rotation * Vec3::new(1.0, -1.0, -1.0), translation: self.translation * Vec3::new(-1.0, 1.0, 1.0), ..self }
    }

    /// `firstperson_righthand` or `firstperson_lefthand`; `blocking` raises a shield.
    pub fn first_person(name: &str, form: Form, left: bool, blocking: bool) -> Display {
        // Entries without a left hand of their own take the right one (`block/block`).
        let listed = match (name, form, left) {
            (_, Form::Shield, _) => shield::display(true, left, blocking),
            ("minecraft:bow", _, false) => Display::px([-80.0, 260.0, -40.0], [-1.0, -2.0, 2.5], 0.9),
            ("minecraft:bow", _, true) => Display::px([-80.0, -280.0, 40.0], [-1.0, -2.0, 2.5], 0.9),
            (_, Form::Block, _) => Display::px([0.0, 45.0, 0.0], [0.0; 3], 0.4),
            (_, Form::Flat, false) => Display::px([0.0, -90.0, 25.0], [1.13, 3.2, 1.13], 0.68),
            (_, Form::Flat, true) => Display::px([0.0, 90.0, -25.0], [1.13, 3.2, 1.13], 0.68),
        };
        if left { listed.mirrored() } else { listed }
    }

    /// `thirdperson_righthand` or `thirdperson_lefthand`. Flat items take `item/handheld`'s,
    /// which mobs mostly hold; its left entry mirrors back to the right one's turn.
    pub fn third_person(form: Form, left: bool) -> Display {
        let listed = match (form, left) {
            (Form::Shield, _) => shield::display(false, left, false),
            (Form::Block, _) => Display::px([75.0, 45.0, 0.0], [0.0, 2.5, 0.0], 0.375),
            (Form::Flat, false) => Display::px([0.0, -90.0, 55.0], [0.0, 4.0, 0.5], 0.85),
            (Form::Flat, true) => Display::px([0.0, 90.0, -55.0], [0.0, 4.0, 0.5], 0.85),
        };
        if left { listed.mirrored() } else { listed }
    }

    /// Java's item space (the model's unit box, centred) to the hand's.
    pub(super) fn frame(self) -> Mat4 {
        let [rx, ry, rz] = self.rotation.to_array().map(f32::to_radians);
        Mat4::from_translation(self.translation) * Mat4::from_euler(EulerRot::XYZ, rx, ry, rz) * Mat4::from_scale(Vec3::splat(self.scale))
    }
}

/// The item in the camera's right or `left` hand. `swing` is 0 to 1 through an arm swing (0 at rest).
pub fn first_person(model: &ItemModel, display: Display, camera: &Camera, left: bool, swing: f32, using: Option<Using>) -> EntityInstance {
    let forward = camera.forward();
    let right = camera.right();
    let up = right.cross(forward);
    let mut view = Mat4::from_mat3(Mat3::from_cols(right, up, -forward));
    if let Some(Using::Eat { ticks, duration }) = using {
        view *= eat_transform(ticks, duration);
    }
    // Java's swing: the arm dips and turns in, by sin of the progress.
    let s = (swing.clamp(0.0, 1.0) * std::f32::consts::PI).sin();
    let side = if left { -1.0 } else { 1.0 };
    let arm = (ARM + Vec3::new(-0.4 * s, 0.2 * s, -0.2 * s)) * Vec3::new(side, 1.0, 1.0);
    let drawn = match using {
        Some(Using::Bow { ticks }) => bow_transform(ticks),
        _ => Mat4::IDENTITY,
    };
    let frame = view * Mat4::from_translation(arm) * drawn * Mat4::from_rotation_y(-s * 0.35 * side) * display.frame() * Mat4::from_scale(MESH);
    EntityInstance {
        layers: model.layers.clone(),
        skin: Some(model.skin.clone()),
        position: camera.position,
        yaw: 0.0,
        scale: display.scale,
        pose: Pose::default(),
        frame: Some(frame),
        hurt: false,
        glint: model.glint.then_some(Glint::Item),
    }
}
