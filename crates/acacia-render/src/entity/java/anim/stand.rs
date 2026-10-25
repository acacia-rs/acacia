//! `ArmorStandArmorModel` and `ArmorStandModel` (26.3 client): each limb at its stand pose, in
//! degrees; the base plate turned against the stand.

use super::super::mth::pi_div;
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::humanoid::{Humanoid, Model, humanoid};

/// `ArmorStand.DEFAULT_*_POSE`, by part, and the render state field that overrides it.
const POSES: [(&str, &str, [f32; 3]); 6] = [
    ("head", "headPose", [0.0, 0.0, 0.0]),
    ("body", "bodyPose", [0.0, 0.0, 0.0]),
    ("left_arm", "leftArmPose", [-10.0, 0.0, -10.0]),
    ("right_arm", "rightArmPose", [-15.0, 0.0, 10.0]),
    ("left_leg", "leftLegPose", [-1.0, 0.0, -1.0]),
    ("right_leg", "rightLegPose", [1.0, 0.0, 1.0]),
];

fn degrees(s: &RenderState, field: &str, default: [f32; 3]) -> [f32; 3] {
    let axis = |i: usize, c: &str| if s.has(&format!("{field}{c}")) { s.num(&format!("{field}{c}")) } else { default[i] };
    [axis(0, "X"), axis(1, "Y"), axis(2, "Z")]
}

pub(super) fn armor_stand(p: &mut Poses, s: &RenderState) {
    humanoid(p, s, &Humanoid::of(s), Model::Humanoid);
    let radians = |d: f32| pi_div(180.0) * d;
    for (part, field, default) in POSES {
        let [x, y, z] = degrees(s, field, default);
        let pose = p.get(part);
        (pose.x_rot, pose.y_rot, pose.z_rot) = (radians(x), radians(y), radians(z));
    }
    p.y_rot("base_plate", radians(-s.y_rot()));
    // ArmorStandModel's constructor hides the hat; the armour's ArmorStandArmorModel keeps it.
    if p.has("base_plate") {
        p.set_visible("hat", false);
    }
    let arms = s.flag("showArms");
    p.set_visible("left_arm", arms);
    p.set_visible("right_arm", arms);
    p.set_visible("base_plate", !s.has("showBasePlate") || s.flag("showBasePlate"));
    let [x, y, z] = degrees(s, "bodyPose", [0.0; 3]);
    for stick in ["right_body_stick", "left_body_stick", "shoulder_stick"] {
        let pose = p.get(stick);
        (pose.x_rot, pose.y_rot, pose.z_rot) = (radians(x), radians(y), radians(z));
    }
}
