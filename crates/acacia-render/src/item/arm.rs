//! The player's own arms in first person: the right one while the main hand is empty, and those
//! under a held map. Java's `ItemInHandRenderer` (`renderPlayerArm`, `renderMapHand`), drawing
//! the player model with every other bone hidden.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3};

use super::map::{self, Hold};
use crate::Camera;
use crate::entity::{EntityInstance, Layer, Pose, Skin};

/// The bones of each arm and its sleeve, as meshes name them (lowercase): right, left.
const ARMS: [[&str; 2]; 2] = [["rightarm", "rightsleeve"], ["leftarm", "leftsleeve"]];
/// A humanoid's neck, where Java's model space has its origin, in blocks over the feet.
const NECK: f32 = 1.5;

/// The player the arms belong to: its layers and skin, and its mesh's bones in order.
pub struct Wearer<'a> {
    pub layers: &'a [Layer],
    pub skin: Option<Arc<Skin>>,
    pub bones: &'a [String],
}

/// `renderPlayerArm`: Java's model space (y down from the neck, 1/16 block) placed in view
/// space. `swing` is 0 to 1 through an arm swing; `side` is 1 for the right arm, -1 for the left.
fn bare(swing: f32, side: f32) -> Mat4 {
    let root = swing.sqrt();
    let at = Vec3::new(side * (-0.3 * (root * PI).sin() + 0.640_000_05), 0.4 * (root * TAU).sin() - 0.6, -0.4 * (swing * PI).sin() - 0.719_999_97);
    Mat4::from_translation(at)
        * Mat4::from_rotation_y((side * 45.0).to_radians())
        * Mat4::from_rotation_y((side * (root * PI).sin() * 70.0).to_radians())
        * Mat4::from_rotation_z((side * (swing * swing * PI).sin() * -20.0).to_radians())
        * Mat4::from_translation(Vec3::new(-side, 3.6, 3.5))
        * Mat4::from_rotation_z((side * 120.0).to_radians())
        * Mat4::from_rotation_x(200f32.to_radians())
        * Mat4::from_rotation_y((side * -135.0).to_radians())
        * Mat4::from_translation(Vec3::new(side * 5.6, 0.0, 0.0))
}

/// `renderMapHand`, under a map lying in both hands.
fn under_map(pitch: f32, side: f32) -> Mat4 {
    map::lying(pitch)
        * Mat4::from_rotation_y(90f32.to_radians())
        * Mat4::from_rotation_y(92f32.to_radians())
        * Mat4::from_rotation_x(45f32.to_radians())
        * Mat4::from_rotation_z((side * -41.0).to_radians())
        * Mat4::from_translation(Vec3::new(side * 0.3, -1.1, 0.45))
}

/// The pack's model space (y up from the feet) to Java's.
fn to_java() -> Mat4 {
    Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)) * Mat4::from_translation(Vec3::new(0.0, -NECK, 0.0))
}

fn arm(wearer: &Wearer, camera: &Camera, left: bool, placed: Mat4) -> EntityInstance {
    let mut hidden = [0u32; 4];
    let shown = ARMS[usize::from(left)];
    for (index, _) in wearer.bones.iter().enumerate().take(128).filter(|(_, bone)| !shown.contains(&bone.as_str())) {
        hidden[index / 32] |= 1 << (index % 32);
    }
    let bared = |layer: &Layer| Layer { hidden: std::array::from_fn(|word| layer.hidden[word] | hidden[word]), ..layer.clone() };
    let (forward, right) = (camera.forward(), camera.right());
    let view = Mat4::from_mat3(Mat3::from_cols(right, right.cross(forward), -forward));
    EntityInstance {
        layers: wearer.layers.iter().map(bared).collect(),
        skin: wearer.skin.clone(),
        position: camera.position,
        yaw: 0.0,
        scale: 1.0,
        pose: Pose::default(),
        frame: Some(view * placed * to_java()),
        hurt: false,
        glint: None,
    }
}

/// The right arm of an empty main hand; `swing` is 0 to 1 through an arm swing.
pub fn first_person(wearer: &Wearer, camera: &Camera, swing: f32) -> EntityInstance {
    arm(wearer, camera, false, bare(swing.clamp(0.0, 1.0), 1.0))
}

/// The arms holding a map: both under one in both hands, else the hand that has it.
pub fn with_map(wearer: &Wearer, camera: &Camera, hold: Hold) -> Vec<EntityInstance> {
    match hold {
        Hold::Both => [(false, 1.0), (true, -1.0)].map(|(left, side)| arm(wearer, camera, left, under_map(map::pitch(camera), side))).into(),
        Hold::One { left } => {
            let side = if left { -1.0 } else { 1.0 };
            let beside = Mat4::from_translation(Vec3::new(side * 0.125, -0.125, 0.0)) * Mat4::from_rotation_z((side * 10.0).to_radians());
            vec![arm(wearer, camera, left, beside * bare(0.0, side))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A point of the pack's model space (px) in view space.
    fn seen(placed: Mat4, x: f32, y: f32) -> Vec3 {
        (placed * to_java()).transform_point3(Vec3::new(x, y, 0.0) / 16.0)
    }

    #[test]
    fn the_resting_arm_reaches_into_the_view_from_the_lower_right() {
        // The arm's box in the pack's space: 4 px wide from the shoulder at 22 px down to 12.
        let (shoulder, fist) = (seen(bare(0.0, 1.0), -6.0, 23.0), seen(bare(0.0, 1.0), -6.0, 12.0));
        // The fist is further in than the shoulder and, on the screen, nearer the middle.
        assert!(fist.z < -0.3 && fist.z < shoulder.z, "{shoulder} {fist}");
        let on_screen = |p: Vec3| (p.x / -p.z, p.y / -p.z);
        let ((sx, sy), (fx, fy)) = (on_screen(shoulder), on_screen(fist));
        assert!(fx > 0.0 && fx < sx && fy < 0.0 && fy > sy, "{shoulder} {fist}");
        // The left arm mirrors it.
        let left = seen(bare(0.0, -1.0), 6.0, 12.0);
        assert!((left - fist * Vec3::new(-1.0, 1.0, 1.0)).abs().max_element() < 1e-4, "{fist} {left}");
    }

    #[test]
    fn the_arms_under_a_map_come_from_either_side() {
        // Looking down, the map is upright before the face: a fist to each side of its middle.
        let (right, left) = (seen(under_map(60.0, 1.0), -6.0, 12.0), seen(under_map(60.0, -1.0), 6.0, 12.0));
        // Java turns them 92°, not 90°: not quite mirrored.
        assert!(right.x > 0.2 && (right.x + left.x).abs() < 0.05 && (right.y - left.y).abs() < 1e-3 && right.z < 0.0, "{right} {left}");
    }
}
