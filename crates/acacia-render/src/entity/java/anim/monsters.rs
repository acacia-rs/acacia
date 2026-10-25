//! `EndermanModel`, `WitchModel` and `VexModel` (26.3 client).

use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;
use super::humanoid::{Humanoid, Model, humanoid};

/// A humanoid's swing halved and kept within 0.4 rad; arms raised while carrying a block; the
/// head lifted out of the hat when creepy.
pub(super) fn enderman(p: &mut Poses, s: &RenderState) {
    humanoid(p, s, &Humanoid::of(s), Model::Humanoid);
    p.set_visible("head", true);
    for limb in ["right_arm", "left_arm", "right_leg", "left_leg"] {
        let x = p.read(limb).x_rot * 0.5;
        p.x_rot(limb, x.clamp(-0.4, 0.4));
    }
    if s.flag("carriedBlock") {
        p.x_rot("right_arm", -0.5);
        p.x_rot("left_arm", -0.5);
        p.z_rot("right_arm", 0.05);
        p.z_rot("left_arm", -0.05);
    }
    if s.flag("isCreepy") {
        p.get("head").y -= 5.0;
        p.get("hat").y += 5.0;
    }
}

/// The nose twitches at a pace set by the entity id; it points down at a held potion.
pub(super) fn witch(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    p.x_rot("right_leg", cos(pos * 0.6662) * 1.4 * speed * 0.5);
    p.x_rot("left_leg", cos(pos * 0.6662 + std::f32::consts::PI) * 1.4 * speed * 0.5);
    let pace = 0.01 * (s.num("entityId") as i32 % 10) as f32;
    p.x_rot("nose", sin(s.age() * pace) * 4.5 * pi_div(180.0));
    p.z_rot("nose", cos(s.age() * pace) * 2.5 * pi_div(180.0));
    if s.flag("isHoldingItem") {
        let nose = p.get("nose");
        (nose.x, nose.y, nose.z) = (0.0, 1.0, -1.5);
        nose.x_rot = -0.9;
    }
}

/// Arms out and bobbing, raised to charge; the wings beat.
pub(super) fn vex(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let bob = cos(s.age() * 5.5 * pi_div(180.0)) * 0.1;
    p.z_rot("right_arm", pi_div(5.0) + bob);
    p.z_rot("left_arm", -(pi_div(5.0) + bob));
    if s.flag("isCharging") {
        p.x_rot("body", 0.0);
        let (right, left) = (s.flag("rightHandItemState"), s.flag("leftHandItemState"));
        let (x_rot, y_rot) = if right || left { ((std::f64::consts::PI * 7.0 / 6.0) as f32, pi_div(12.0)) } else { (-1.221_730_5, pi_div(12.0)) };
        if right || !left {
            (p.get("right_arm").x_rot, p.get("right_arm").y_rot, p.get("right_arm").z_rot) = (x_rot, y_rot, -0.471_238_88 - bob);
        }
        if left || !right {
            (p.get("left_arm").x_rot, p.get("left_arm").y_rot, p.get("left_arm").z_rot) = (x_rot, -y_rot, 0.471_238_88 + bob);
        }
    } else {
        p.x_rot("body", pi_div(20.0));
    }
    let beat = 1.099_557_4 + cos(s.age() * 45.836_624 * pi_div(180.0)) * pi_div(180.0) * 16.2;
    p.y_rot("left_wing", beat);
    p.y_rot("right_wing", -beat);
    p.x_rot("left_wing", 0.471_238_88);
    p.z_rot("left_wing", -0.471_238_88);
    p.x_rot("right_wing", 0.471_238_88);
    p.z_rot("right_wing", 0.471_238_88);
}
