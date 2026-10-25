//! `HumanoidModel.ArmPose` and the arm poses `poseRightArm` / `poseLeftArm` and
//! `AnimationUtils` set. Ported from Horizon's `mobs/anim.rs` (MIT).

use std::f32::consts::{FRAC_PI_6, PI};

use super::super::mth::{cos, lerp, sin};
use super::super::poses::Poses;
use super::bob;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmPose {
    Empty,
    Item,
    Block,
    Bow,
    CrossbowHold,
    CrossbowCharge,
    ThrowTrident,
}

impl ArmPose {
    /// By the Java constant's name; one this port lacks reads as `EMPTY`.
    pub fn named(name: Option<&str>) -> ArmPose {
        match name.unwrap_or("EMPTY") {
            "ITEM" => ArmPose::Item,
            "BLOCK" => ArmPose::Block,
            "BOW_AND_ARROW" => ArmPose::Bow,
            "CROSSBOW_HOLD" => ArmPose::CrossbowHold,
            "CROSSBOW_CHARGE" => ArmPose::CrossbowCharge,
            "THROW_TRIDENT" => ArmPose::ThrowTrident,
            // TODO: SPYGLASS, TOOT_HORN, BRUSH, once the harness has their cases.
            _ => ArmPose::Empty,
        }
    }

    pub(super) fn two_handed(self) -> bool {
        matches!(self, ArmPose::Bow | ArmPose::CrossbowHold | ArmPose::CrossbowCharge)
    }

    pub(super) fn affects_offhand(self) -> bool {
        self.two_handed() || self == ArmPose::ThrowTrident
    }
}

/// `AnimationUtils.swingWeaponDown` with the right arm as the main arm.
pub(super) fn swing_weapon_down(p: &mut Poses, swing: f32, age: f32) {
    let attack2 = sin(swing * PI);
    let attack = sin((1.0 - (1.0 - swing) * (1.0 - swing)) * PI);
    p.z_rot("right_arm", 0.0);
    p.z_rot("left_arm", 0.0);
    p.y_rot("right_arm", PI / 20.0);
    p.y_rot("left_arm", -PI / 20.0);
    p.x_rot("right_arm", -1.884_955_8 + cos(age * 0.09) * 0.15);
    p.x_rot("left_arm", -0.0 + cos(age * 0.19) * 0.5);
    p.get("right_arm").x_rot += attack2 * 2.2 - attack * 0.4;
    p.get("left_arm").x_rot += attack2 * 1.2 - attack * 0.4;
    bob(p, "right_arm", age, 1.0);
    bob(p, "left_arm", age, -1.0);
}

/// `AnimationUtils.animateCrossbowHold`.
fn crossbow_hold(p: &mut Poses, hx: f32, hy: f32, in_right: bool) {
    let (hold, shoot) = if in_right { ("right_arm", "left_arm") } else { ("left_arm", "right_arm") };
    p.y_rot(hold, if in_right { -0.3 } else { 0.3 } + hy);
    p.y_rot(shoot, if in_right { 0.6 } else { -0.6 } + hy);
    p.x_rot(hold, -PI / 2.0 + hx + 0.1);
    p.x_rot(shoot, -1.5 + hx);
}

/// `AnimationUtils.animateCrossbowCharge`.
fn crossbow_charge(p: &mut Poses, (max, ticks): (f32, f32), in_right: bool) {
    let (hold, pull) = if in_right { ("right_arm", "left_arm") } else { ("left_arm", "right_arm") };
    p.y_rot(hold, if in_right { -0.8 } else { 0.8 });
    p.x_rot(hold, -0.970_796_35);
    p.x_rot(pull, -0.970_796_35);
    let alpha = ticks.clamp(0.0, max) / max;
    p.y_rot(pull, lerp(alpha, 0.4, 0.85) * if in_right { 1.0 } else { -1.0 });
    let x = lerp(alpha, p.read(pull).x_rot, -PI / 2.0);
    p.x_rot(pull, x);
}

/// `HumanoidModel.poseRightArm` / `poseLeftArm`; `charge` is `maxCrossbowChargeDuration`,
/// `ticksUsingItem`.
pub(super) fn pose_arm(p: &mut Poses, pose: ArmPose, right: bool, charge: (f32, f32)) {
    let arm = if right { "right_arm" } else { "left_arm" };
    let (hx, hy) = (p.read("head").x_rot, p.read("head").y_rot);
    match pose {
        ArmPose::Empty => p.y_rot(arm, 0.0),
        ArmPose::Item => {
            let x = p.read(arm).x_rot;
            p.x_rot(arm, x * 0.5 - PI / 10.0);
            p.y_rot(arm, 0.0);
        }
        ArmPose::Block => {
            let x = p.read(arm).x_rot * 0.5 - 0.942_477_9 + hx.clamp(-1.396_263_4, 0.436_332_32);
            p.x_rot(arm, x);
            p.y_rot(arm, if right { -FRAC_PI_6 } else { FRAC_PI_6 } + hy.clamp(-FRAC_PI_6, FRAC_PI_6));
        }
        ArmPose::Bow => {
            p.y_rot("right_arm", -0.1 + hy - if right { 0.0 } else { 0.4 });
            p.y_rot("left_arm", 0.1 + hy + if right { 0.4 } else { 0.0 });
            p.x_rot("right_arm", -PI / 2.0 + hx);
            p.x_rot("left_arm", -PI / 2.0 + hx);
        }
        ArmPose::ThrowTrident => {
            let x = p.read(arm).x_rot;
            p.x_rot(arm, x * 0.5 - PI);
            p.y_rot(arm, 0.0);
        }
        ArmPose::CrossbowHold => crossbow_hold(p, hx, hy, right),
        ArmPose::CrossbowCharge => crossbow_charge(p, charge, right),
    }
}
