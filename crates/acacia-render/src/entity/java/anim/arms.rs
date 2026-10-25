//! `HumanoidModel.ArmPose`, `poseRightArm` / `poseLeftArm` and the arm helpers of
//! `AnimationUtils` (26.3 client).

use std::f32::consts::PI;

use super::super::mth::{cos, lerp, pi_div, sin};
use super::super::poses::Poses;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmPose {
    Empty,
    Item,
    Block,
    Bow,
    ThrowTrident,
    CrossbowCharge,
    CrossbowHold,
    Spyglass,
    TootHorn,
    Brush,
    Spear,
}

impl ArmPose {
    /// By the Java constant's name; `EMPTY` for anything else.
    pub fn named(name: Option<&str>) -> ArmPose {
        match name.unwrap_or("EMPTY") {
            "ITEM" => ArmPose::Item,
            "BLOCK" => ArmPose::Block,
            "BOW_AND_ARROW" => ArmPose::Bow,
            "THROW_TRIDENT" => ArmPose::ThrowTrident,
            "CROSSBOW_CHARGE" => ArmPose::CrossbowCharge,
            "CROSSBOW_HOLD" => ArmPose::CrossbowHold,
            "SPYGLASS" => ArmPose::Spyglass,
            "TOOT_HORN" => ArmPose::TootHorn,
            "BRUSH" => ArmPose::Brush,
            "SPEAR" => ArmPose::Spear,
            _ => ArmPose::Empty,
        }
    }

    pub(super) fn two_handed(self) -> bool {
        matches!(self, ArmPose::Bow | ArmPose::CrossbowCharge | ArmPose::CrossbowHold)
    }

    pub(super) fn affects_offhand(self) -> bool {
        self.two_handed() || self == ArmPose::ThrowTrident
    }
}

pub(super) fn arm(right: bool) -> &'static str {
    if right { "right_arm" } else { "left_arm" }
}

/// `AnimationUtils.bobModelPart`.
pub(super) fn bob(p: &mut Poses, name: &str, age: f32, scale: f32) {
    let part = p.get(name);
    part.z_rot += scale * (cos(age * 0.09) * 0.05 + 0.05);
    part.x_rot += scale * (sin(age * 0.067) * 0.05);
}

/// `AnimationUtils.bobArms`.
pub(super) fn bob_arms(p: &mut Poses, age: f32) {
    bob(p, "right_arm", age, 1.0);
    bob(p, "left_arm", age, -1.0);
}

/// `AnimationUtils.animateCrossbowHold`.
fn crossbow_hold(p: &mut Poses, in_right: bool) {
    let head = p.read("head");
    let (hold, shoot) = (arm(in_right), arm(!in_right));
    p.y_rot(hold, if in_right { -0.3 } else { 0.3 } + head.y_rot);
    p.y_rot(shoot, if in_right { 0.6 } else { -0.6 } + head.y_rot);
    p.x_rot(hold, -PI / 2.0 + head.x_rot + 0.1);
    p.x_rot(shoot, -1.5 + head.x_rot);
}

/// `AnimationUtils.animateCrossbowCharge`.
fn crossbow_charge(p: &mut Poses, (max, ticks): (f32, f32), in_right: bool) {
    let (hold, pull) = (arm(in_right), arm(!in_right));
    p.y_rot(hold, if in_right { -0.8 } else { 0.8 });
    p.x_rot(hold, -0.970_796_35);
    p.x_rot(pull, -0.970_796_35);
    let alpha = ticks.clamp(0.0, max) / max;
    p.y_rot(pull, lerp(alpha, 0.4, 0.85) * if in_right { 1.0 } else { -1.0 });
    let x = lerp(alpha, p.read(pull).x_rot, -PI / 2.0);
    p.x_rot(pull, x);
}

/// `HumanoidModel.poseRightArm` / `poseLeftArm`. `charge`: `maxCrossbowChargeDuration`,
/// `ticksUsingItem`.
pub(super) fn pose_arm(p: &mut Poses, pose: ArmPose, right: bool, crouching: bool, charge: (f32, f32)) {
    let this = arm(right);
    let head = p.read("head");
    let side = if right { -1.0 } else { 1.0 };
    match pose {
        ArmPose::Empty => p.y_rot(this, 0.0),
        ArmPose::Item | ArmPose::Brush => {
            let lowered = if pose == ArmPose::Item { pi_div(10.0) } else { pi_div(5.0) };
            let x = p.read(this).x_rot;
            p.x_rot(this, x * 0.5 - lowered);
            p.y_rot(this, 0.0);
        }
        // poseBlockingArm
        ArmPose::Block => {
            let x = p.read(this).x_rot * 0.5 - 0.942_477_9 + head.x_rot.clamp((-std::f64::consts::PI * 4.0 / 9.0) as f32, 0.436_332_32);
            p.x_rot(this, x);
            p.y_rot(this, side * 30.0 * pi_div(180.0) + head.y_rot.clamp(pi_div(-6.0), pi_div(6.0)));
        }
        ArmPose::Bow => {
            let (right_extra, left_extra) = if right { (0.0, 0.4) } else { (-0.4, 0.0) };
            p.y_rot("right_arm", -0.1 + head.y_rot + right_extra);
            p.y_rot("left_arm", 0.1 + head.y_rot + left_extra);
            p.x_rot("right_arm", -PI / 2.0 + head.x_rot);
            p.x_rot("left_arm", -PI / 2.0 + head.x_rot);
        }
        ArmPose::ThrowTrident => {
            let x = p.read(this).x_rot;
            p.x_rot(this, x * 0.5 - PI);
            p.y_rot(this, 0.0);
        }
        ArmPose::CrossbowCharge => crossbow_charge(p, charge, right),
        ArmPose::CrossbowHold => crossbow_hold(p, right),
        ArmPose::Spyglass => {
            let crouch = if crouching { pi_div(12.0) } else { 0.0 };
            p.x_rot(this, (head.x_rot - 1.919_862_2 - crouch).clamp(-2.4, 3.3));
            p.y_rot(this, if right { head.y_rot - pi_div(12.0) } else { head.y_rot + pi_div(12.0) });
        }
        ArmPose::TootHorn => {
            p.x_rot(this, head.x_rot.clamp(-1.2, 1.2) - 1.483_529_8);
            p.y_rot(this, if right { head.y_rot - pi_div(6.0) } else { head.y_rot + pi_div(6.0) });
        }
        // TODO: SpearAnimations.thirdPersonHandUse.
        ArmPose::Spear => {}
    }
}
