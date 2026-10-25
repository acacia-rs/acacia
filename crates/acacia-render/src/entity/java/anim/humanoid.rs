//! `HumanoidModel.setupAnim` and the families built on it: zombies (`animateZombieArms`) and
//! skeletons (their melee pose). Ported from Horizon's `mobs/anim.rs` (MIT); the arm poses are
//! read from the render state as Java's model reads them, the renderer's `getArmPose` choosing
//! them is the caller's (see README "Java entity render state").

use std::f32::consts::{FRAC_PI_4, PI};

use super::super::model::PartPose;
use super::super::mth::{cos, lerp, rot_lerp_rad, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::arms::{ArmPose, pose_arm, swing_weapon_down};
use super::{bob, head};

/// What `HumanoidModel.setupAnim` reads besides the walk and head inputs.
#[derive(Clone, Copy, Debug)]
pub struct Humanoid {
    pub right: ArmPose,
    pub left: ArmPose,
    pub crouching: bool,
    pub passenger: bool,
    pub fall_flying: bool,
    /// The limb swing's divisor while gliding (1 otherwise).
    pub fly_speed_scale: f32,
    /// `Some(aggressive)` for `animateZombieArms`.
    pub zombie: Option<bool>,
    /// The main hand holds something.
    pub holding: bool,
    /// `maxCrossbowChargeDuration`, `ticksUsingItem`.
    pub charge: (f32, f32),
    /// `swingWeaponDown` in place of the plain attack swing (piglins).
    pub weapon_swing: bool,
    pub swim: f32,
    pub using_item: bool,
    /// The arm the current swing moves (`Some(true)` for the right one).
    pub swing_right_arm: Option<bool>,
}

impl Humanoid {
    pub fn of(state: &RenderState) -> Humanoid {
        let speed_scale = state.num("speedValue");
        Humanoid {
            right: ArmPose::named(state.text("rightArmPose")),
            left: ArmPose::named(state.text("leftArmPose")),
            crouching: state.flag("isCrouching"),
            passenger: state.flag("isPassenger"),
            fall_flying: state.flag("isFallFlying"),
            fly_speed_scale: if speed_scale > 0.0 { speed_scale } else { 1.0 },
            zombie: None,
            holding: state.text("mainHandItem").is_some(),
            charge: (state.num("maxCrossbowChargeDuration"), state.num("ticksUsingItem")),
            weapon_swing: false,
            swim: state.num("swimAmount"),
            using_item: state.flag("isUsingItem"),
            swing_right_arm: None,
        }
    }
}

pub fn humanoid(p: &mut Poses, s: &RenderState, h: &Humanoid) {
    head(p, s);
    if h.fall_flying {
        p.x_rot("head", -FRAC_PI_4);
    }
    let (pos, sp) = (s.walk_pos(), s.walk_speed() / h.fly_speed_scale);
    p.x_rot("right_arm", cos(pos * 0.6662 + PI) * 2.0 * sp * 0.5);
    p.x_rot("left_arm", cos(pos * 0.6662) * 2.0 * sp * 0.5);
    p.x_rot("right_leg", cos(pos * 0.6662) * 1.4 * sp);
    p.x_rot("left_leg", cos(pos * 0.6662 + PI) * 1.4 * sp);
    p.y_rot("right_leg", 0.005);
    p.y_rot("left_leg", -0.005);
    p.z_rot("right_leg", 0.005);
    p.z_rot("left_leg", -0.005);
    if h.passenger {
        p.get("right_arm").x_rot += -PI / 5.0;
        p.get("left_arm").x_rot += -PI / 5.0;
        *p.get("right_leg") = PartPose { x_rot: -1.413_716_7, y_rot: PI / 10.0, z_rot: 0.078_539_82, ..p.read("right_leg") };
        *p.get("left_leg") = PartPose { x_rot: -1.413_716_7, y_rot: -PI / 10.0, z_rot: -0.078_539_82, ..p.read("left_leg") };
    }
    // The main arm is the right one: the off hand goes first unless it needs both hands.
    if h.left.two_handed() {
        pose_arm(p, h.right, true, h.charge);
        if !h.right.affects_offhand() {
            pose_arm(p, h.left, false, h.charge);
        }
    } else {
        pose_arm(p, h.left, false, h.charge);
        if !h.left.affects_offhand() {
            pose_arm(p, h.right, true, h.charge);
        }
    }
    attack(p, s, h);
    if let Some(aggressive) = h.zombie {
        zombie_arms(p, s, h, aggressive);
    }
    if h.crouching {
        p.x_rot("body", 0.5);
        p.get("right_arm").x_rot += 0.4;
        p.get("left_arm").x_rot += 0.4;
        p.get("right_leg").z += 4.0;
        p.get("left_leg").z += 4.0;
        p.get("head").y += 4.2;
        p.get("body").y += 3.2;
        p.get("left_arm").y += 3.2;
        p.get("right_arm").y += 3.2;
    }
    bob(p, "right_arm", s.age(), 1.0);
    bob(p, "left_arm", s.age(), -1.0);
    if h.swim > 0.0 {
        swim(p, s.walk_pos(), h);
    }
}

/// `HumanoidModel.setupAttackAnimation`: the swing with the right arm.
fn attack(p: &mut Poses, s: &RenderState, h: &Humanoid) {
    let (swing, hx) = (s.swing(), p.read("head").x_rot);
    if swing <= 0.0 {
        return;
    }
    if h.weapon_swing {
        swing_weapon_down(p, swing, s.age());
        return;
    }
    let body_y = sin(swing.sqrt() * PI * 2.0) * 0.2;
    p.y_rot("body", body_y);
    let age_scale = if s.baby() { 0.5 } else { 1.0 };
    let r = p.get("right_arm");
    r.z = sin(body_y) * 5.0 * age_scale;
    r.x = -cos(body_y) * 5.0 * age_scale;
    let l = p.get("left_arm");
    l.z = -sin(body_y) * 5.0 * age_scale;
    l.x = cos(body_y) * 5.0 * age_scale;
    p.get("right_arm").y_rot += body_y;
    p.get("left_arm").y_rot += body_y;
    p.get("left_arm").x_rot += body_y;
    let eased = 1.0 - (1.0 - swing).powi(4); // Ease.outQuart
    let aa = sin(eased * PI);
    let bb = sin(swing * PI) * -(hx - 0.7) * 0.75;
    let r = p.get("right_arm");
    r.x_rot -= aa * 1.2 + bb;
    r.y_rot += body_y * 2.0;
    r.z_rot += sin(swing * PI) * -0.4;
}

/// `AnimationUtils.animateZombieArms`.
fn zombie_arms(p: &mut Poses, s: &RenderState, h: &Humanoid, aggressive: bool) {
    let swing = s.swing();
    let raise = !s.baby() || !h.holding;
    let drop = if raise { -PI / if aggressive { 1.5 } else { 2.25 } } else { 0.0 };
    let ym = if raise { 1.0 } else { -1.0 } * sin(swing * PI);
    let xm = sin((1.0 - (1.0 - swing) * (1.0 - swing)) * PI);
    let x = drop + ym * 1.2 - xm * 0.4;
    let y = 0.1 - ym * 0.6;
    // An arm raising a trident keeps its pose.
    for (arm, pose, sign) in [("right_arm", h.right, -1.0), ("left_arm", h.left, 1.0)] {
        if pose != ArmPose::ThrowTrident {
            p.x_rot(arm, x);
            p.y_rot(arm, if raise { sign * y } else { -sign * y });
            p.z_rot(arm, 0.0);
        }
    }
}

/// `HumanoidModel.setupSwimAnimation`. The left arm eases the short way round, the right one
/// linearly, as in Java.
fn swim(p: &mut Poses, pos: f32, h: &Humanoid) {
    let amount = h.swim;
    if !h.fall_flying {
        let x = p.read("head").x_rot;
        p.x_rot("head", rot_lerp_rad(amount, x, -FRAC_PI_4));
    }
    let swim_pos = pos % 26.0;
    let right_amount = if h.swing_right_arm == Some(true) { 0.0 } else { amount };
    let left_amount = if h.swing_right_arm == Some(false) { 0.0 } else { amount };
    let quadratic = |x: f32| -65.0 * x + x * x;
    let targets = if h.using_item {
        None
    } else if swim_pos < 14.0 {
        let stroke = 1.870_796_4 * quadratic(swim_pos) / quadratic(14.0);
        Some(([0.0, PI, PI + stroke], [0.0, PI, PI - stroke]))
    } else if swim_pos < 22.0 {
        let t = (swim_pos - 14.0) / 8.0;
        let x = PI / 2.0 * t;
        Some(([x, PI, 5.012_389 - 1.870_796_4 * t], [x, PI, 1.270_796_3 + 1.870_796_4 * t]))
    } else if swim_pos < 26.0 {
        let x = PI / 2.0 - PI / 2.0 * ((swim_pos - 22.0) / 4.0);
        Some(([x, PI, PI], [x, PI, PI]))
    } else {
        None
    };
    if let Some((left, right)) = targets {
        let l = p.get("left_arm");
        for (angle, target) in [&mut l.x_rot, &mut l.y_rot, &mut l.z_rot].into_iter().zip(left) {
            *angle = rot_lerp_rad(left_amount, *angle, target);
        }
        let r = p.get("right_arm");
        for (angle, target) in [&mut r.x_rot, &mut r.y_rot, &mut r.z_rot].into_iter().zip(right) {
            *angle = lerp(right_amount, *angle, target);
        }
    }
    let x = p.read("left_leg").x_rot;
    p.x_rot("left_leg", lerp(amount, x, 0.3 * cos(pos * 0.333_333_34 + PI)));
    let x = p.read("right_leg").x_rot;
    p.x_rot("right_leg", lerp(amount, x, 0.3 * cos(pos * 0.333_333_34)));
}

/// `SkeletonModel.setupAnim`'s melee pose, for an aggressive skeleton without a bow.
pub(super) fn skeleton_melee(p: &mut Poses, s: &RenderState) {
    let sw = s.swing();
    let s2 = sin(sw * PI);
    let s1 = sin((1.0 - (1.0 - sw) * (1.0 - sw)) * PI);
    p.z_rot("right_arm", 0.0);
    p.z_rot("left_arm", 0.0);
    p.y_rot("right_arm", -(0.1 - s2 * 0.6));
    p.y_rot("left_arm", 0.1 - s2 * 0.6);
    p.x_rot("right_arm", -PI / 2.0 - (s2 * 1.2 - s1 * 0.4));
    p.x_rot("left_arm", -PI / 2.0 - (s2 * 1.2 - s1 * 0.4));
    bob(p, "right_arm", s.age(), 1.0);
    bob(p, "left_arm", s.age(), -1.0);
}
