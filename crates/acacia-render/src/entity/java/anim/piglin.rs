//! `AbstractPiglinModel` (ears flapping with the walk) and its subclasses: `PiglinModel`'s arm
//! poses (dancing, a raised weapon, crossbows, admiring gold) and `ZombifiedPiglinModel`'s
//! zombie arms. Ported from the 26.3 client's models.

use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::arms::{crossbow_charge, crossbow_hold};
use super::humanoid::{Humanoid, Model, humanoid};

/// `getDefaultEarAngleInDegrees`: an adult's ears stand out 30°, a baby's 5°.
fn ears(p: &mut Poses, s: &RenderState) {
    let default = if s.baby() { 5.0 } else { 30.0 } * pi_div(180.0);
    let frequency = s.age() * 0.1 + s.walk_pos() * 0.5;
    let amplitude = 0.08 + s.walk_speed() * 0.4;
    p.z_rot("left_ear", -default - cos(frequency * 1.2) * amplitude);
    p.z_rot("right_ear", default + cos(frequency) * amplitude);
}

pub(super) fn zombified(p: &mut Poses, s: &RenderState) {
    humanoid(p, s, &Humanoid::of(s), Model::Zombie);
    ears(p, s);
}

pub(super) fn piglin(p: &mut Poses, s: &RenderState) {
    let h = Humanoid::of(s);
    humanoid(p, s, &h, Model::Humanoid);
    ears(p, s);
    match s.text("armPose") {
        Some("DANCING") => {
            let dance = s.age() / 60.0;
            p.z_rot("right_ear", pi_div(6.0) + pi_div(180.0) * sin(dance * 30.0) * 10.0);
            p.z_rot("left_ear", -pi_div(6.0) - pi_div(180.0) * cos(dance * 30.0) * 10.0);
            p.get("head").x += sin(dance * 10.0);
            p.get("head").y += sin(dance * 40.0) + 0.4;
            let arm = pi_div(180.0) * (70.0 + cos(dance * 40.0) * 10.0);
            p.z_rot("right_arm", arm);
            p.z_rot("left_arm", arm * -1.0);
            p.get("right_arm").y += sin(dance * 40.0) * 0.5 - 0.5;
            p.get("left_arm").y += sin(dance * 40.0) * 0.5 + 0.5;
            p.get("body").y += sin(dance * 40.0) * 0.35;
        }
        Some("ATTACKING_WITH_MELEE_WEAPON") if s.swing() == 0.0 => p.x_rot(if h.main_right { "right_arm" } else { "left_arm" }, -1.8),
        Some("CROSSBOW_HOLD") => crossbow_hold(p, h.main_right),
        Some("CROSSBOW_CHARGE") => crossbow_charge(p, (s.num("maxCrossbowChageDuration"), s.num("ticksUsingItem")), h.main_right),
        Some("ADMIRING_ITEM") => {
            p.x_rot("head", 0.5);
            p.y_rot("head", 0.0);
            let (arm, turn) = if h.main_right { ("left_arm", 0.5) } else { ("right_arm", -0.5) };
            p.y_rot(arm, turn);
            p.x_rot(arm, -0.9);
        }
        _ => {}
    }
}
