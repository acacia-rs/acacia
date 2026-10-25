//! `IllagerModel.setupAnim` (26.3 client): vindicators, evokers, pillagers, illusioners.

use std::f32::consts::PI;

use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::arms::{bob_arms, crossbow_charge, crossbow_hold};
use super::head;
use super::humanoid::{Humanoid, zombie_arms};

pub(super) fn illager(p: &mut Poses, s: &RenderState) {
    head(p, s);
    p.set_visible("hat", false);
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    let limbs = if s.flag("isRiding") {
        [(pi_div(-5.0), 0.0, 0.0), (pi_div(-5.0), 0.0, 0.0), (-1.413_716_7, pi_div(10.0), 0.078_539_82), (-1.413_716_7, pi_div(-10.0), -0.078_539_82)]
    } else {
        [
            (cos(pos * 0.6662 + PI) * 2.0 * speed * 0.5, 0.0, 0.0),
            (cos(pos * 0.6662) * 2.0 * speed * 0.5, 0.0, 0.0),
            (cos(pos * 0.6662) * 1.4 * speed * 0.5, 0.0, 0.0),
            (cos(pos * 0.6662 + PI) * 1.4 * speed * 0.5, 0.0, 0.0),
        ]
    };
    for (name, (x, y, z)) in ["right_arm", "left_arm", "right_leg", "left_leg"].into_iter().zip(limbs) {
        let part = p.get(name);
        (part.x_rot, part.y_rot, part.z_rot) = (x, y, z);
    }
    let age = s.age();
    let pose = s.text("armPose").unwrap_or("NEUTRAL");
    match pose {
        "ATTACKING" => {
            let h = Humanoid::of(s);
            if s.flag(if h.main_right { "rightHandItemState" } else { "leftHandItemState" }) {
                swing_weapon_down(p, h.main_right, s.swing(), age);
            } else {
                zombie_arms(p, s, &h, true);
                bob_arms(p, age);
            }
        }
        "SPELLCASTING" | "CELEBRATING" => {
            let (lift, right_z) = if pose == "SPELLCASTING" { (0.25, (std::f64::consts::PI * 3.0 / 4.0) as f32) } else { (0.05, 2.670_354) };
            for (name, x, z_rot) in [("right_arm", -5.0, right_z), ("left_arm", 5.0, (-std::f64::consts::PI * 3.0 / 4.0) as f32)] {
                let part = p.get(name);
                (part.x, part.z) = (x, 0.0);
                (part.x_rot, part.y_rot, part.z_rot) = (cos(age * 0.6662) * lift, 0.0, z_rot);
            }
        }
        "BOW_AND_ARROW" => {
            let head = p.read("head");
            p.y_rot("right_arm", -0.1 + head.y_rot);
            p.x_rot("right_arm", -PI / 2.0 + head.x_rot);
            p.x_rot("left_arm", -0.942_477_9 + head.x_rot);
            p.y_rot("left_arm", head.y_rot - 0.4);
            p.z_rot("left_arm", PI / 2.0);
        }
        "CROSSBOW_HOLD" => crossbow_hold(p, true),
        "CROSSBOW_CHARGE" => crossbow_charge(p, (s.num("maxCrossbowChargeDuration"), s.num("ticksUsingItem")), true),
        _ => {}
    }
    let crossed = pose == "CROSSED";
    p.set_visible("arms", crossed);
    p.set_visible("left_arm", !crossed);
    p.set_visible("right_arm", !crossed);
}

/// `AnimationUtils.swingWeaponDown`.
fn swing_weapon_down(p: &mut Poses, main_right: bool, attack: f32, age: f32) {
    let attack2 = sin(attack * PI);
    let eased = sin((1.0 - (1.0 - attack) * (1.0 - attack)) * PI);
    p.z_rot("right_arm", 0.0);
    p.z_rot("left_arm", 0.0);
    p.y_rot("right_arm", pi_div(20.0));
    p.y_rot("left_arm", pi_div(-20.0));
    let (main, off) = if main_right { ("right_arm", "left_arm") } else { ("left_arm", "right_arm") };
    p.x_rot(main, -1.884_955_8 + cos(age * 0.09) * 0.15 + attack2 * 2.2 - eased * 0.4);
    p.x_rot(off, -0.0 + cos(age * 0.19) * 0.5 + attack2 * 1.2 - eased * 0.4);
    bob_arms(p, age);
}
