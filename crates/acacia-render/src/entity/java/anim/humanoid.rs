//! `HumanoidModel.setupAnim` (26.3 client) and the families built on it: zombies
//! (`animateZombieArms` in their attack animation) and skeletons (their melee pose). The arm
//! poses are read from the render state, as the model reads them; the renderer's `getArmPose`
//! choosing them is in `bedrock.rs`.

use std::f32::consts::{FRAC_PI_4, PI};

use super::super::model::PartPose;
use super::super::mth::{cos, lerp, pi_div, rot_lerp_rad, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::arms::{ArmPose, arm, bob, bob_arms, pose_arm};
use super::head;

/// What `HumanoidModel.setupAnim` reads besides the walk and head inputs.
#[derive(Clone, Copy, Debug)]
pub struct Humanoid {
    pub right: ArmPose,
    pub left: ArmPose,
    /// `mainArm == RIGHT`.
    pub main_right: bool,
    /// `useItemHand == MAIN_HAND`.
    pub use_main_hand: bool,
    pub crouching: bool,
    pub passenger: bool,
    pub fall_flying: bool,
    pub speed_value: f32,
    pub using_item: bool,
    pub swim: f32,
    /// `maxCrossbowChargeDuration`, `ticksUsingItem`.
    pub charge: (f32, f32),
    /// `getMainHandItemStack()` is not empty.
    pub holding: bool,
    /// `currentSwing`'s arm (`true` for the right one), when there is one.
    pub swing_right: Option<bool>,
}

impl Humanoid {
    pub fn of(s: &RenderState) -> Humanoid {
        let main_right = s.text("mainArm") != Some("LEFT");
        let main_hand_item = if main_right { "rightHandItemStack" } else { "leftHandItemStack" };
        let swing_right = s.text("currentSwing").map(|hand| (hand == "MAIN_HAND") == main_right);
        Humanoid {
            right: ArmPose::named(s.text("rightArmPose")),
            left: ArmPose::named(s.text("leftArmPose")),
            main_right,
            use_main_hand: s.text("useItemHand") != Some("OFF_HAND"),
            crouching: s.flag("isCrouching"),
            passenger: s.flag("isPassenger"),
            fall_flying: s.flag("isFallFlying"),
            speed_value: if s.has("speedValue") { s.num("speedValue") } else { 1.0 },
            using_item: s.flag("isUsingItem"),
            swim: s.num("swimAmount"),
            charge: (s.num("maxCrossbowChargeDuration"), s.num("ticksUsingItem")),
            holding: s.text(main_hand_item).is_some_and(|item| item != "minecraft:air"),
            swing_right,
        }
    }
}

/// Which `HumanoidModel` subclass: the overrides of `setupAttackAnimation` and `setupSwimAnimation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Humanoid,
    /// `ZombieModel`: `animateZombieArms` after the attack animation.
    Zombie,
    /// `DrownedModel`: the zombie's arms, its own swim.
    Drowned,
}

pub fn humanoid(p: &mut Poses, s: &RenderState, h: &Humanoid, model: Model) {
    head(p, s);
    if h.fall_flying {
        p.x_rot("head", -FRAC_PI_4);
    }
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    p.x_rot("right_arm", cos(pos * 0.6662 + PI) * 2.0 * speed * 0.5 / h.speed_value);
    p.x_rot("left_arm", cos(pos * 0.6662) * 2.0 * speed * 0.5 / h.speed_value);
    p.x_rot("right_leg", cos(pos * 0.6662) * 1.4 * speed / h.speed_value);
    p.x_rot("left_leg", cos(pos * 0.6662 + PI) * 1.4 * speed / h.speed_value);
    p.y_rot("right_leg", 0.005);
    p.y_rot("left_leg", -0.005);
    p.z_rot("right_leg", 0.005);
    p.z_rot("left_leg", -0.005);
    if h.passenger {
        p.get("right_arm").x_rot += pi_div(-5.0);
        p.get("left_arm").x_rot += pi_div(-5.0);
        *p.get("right_leg") = PartPose { x_rot: -1.413_716_7, y_rot: pi_div(10.0), z_rot: 0.078_539_82, ..p.read("right_leg") };
        *p.get("left_leg") = PartPose { x_rot: -1.413_716_7, y_rot: pi_div(-10.0), z_rot: -0.078_539_82, ..p.read("left_leg") };
    }
    let first_right = if h.using_item {
        h.use_main_hand == h.main_right
    } else {
        let off_hand_right = !h.main_right;
        let off_pose = if off_hand_right { h.right } else { h.left };
        if off_pose.two_handed() { h.main_right } else { off_hand_right }
    };
    let (first, second) = if first_right { ((h.right, true), (h.left, false)) } else { ((h.left, false), (h.right, true)) };
    pose_arm(p, first.0, first.1, h.crouching, h.charge);
    if !first.0.affects_offhand() {
        pose_arm(p, second.0, second.1, h.crouching, h.charge);
    }
    attack(p, s, h);
    if model != Model::Humanoid {
        zombie_arms(p, s, h, s.flag("isAggressive"));
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
    if h.right != ArmPose::Spyglass {
        bob(p, "right_arm", s.age(), 1.0);
    }
    if h.left != ArmPose::Spyglass {
        bob(p, "left_arm", s.age(), -1.0);
    }
    if h.swim > 0.0 {
        if model == Model::Drowned {
            drowned_swim(p, s.age(), h.swim);
        } else {
            swim(p, pos, h);
        }
    }
}

/// `DrownedModel.setupSwimAnimation`.
fn drowned_swim(p: &mut Poses, age: f32, amount: f32) {
    let reach = (-std::f64::consts::PI * 4.0 / 5.0) as f32;
    let paddle = amount * 0.35 * sin(0.1 * age);
    let r = p.get("right_arm");
    r.x_rot = rot_lerp_rad(amount, r.x_rot, reach) + paddle;
    r.z_rot = rot_lerp_rad(amount, r.z_rot, -0.15);
    let l = p.get("left_arm");
    l.x_rot = rot_lerp_rad(amount, l.x_rot, reach) - paddle;
    l.z_rot = rot_lerp_rad(amount, l.z_rot, 0.15);
    let kick = amount * 0.55 * sin(0.1 * age);
    p.get("left_leg").x_rot -= kick;
    p.get("right_leg").x_rot += kick;
    p.x_rot("head", 0.0);
}

/// `HumanoidModel.setupAttackAnimation` with a `WHACK` swing.
// TODO: STAB swings (SpearAnimations.thirdPersonAttackHand).
fn attack(p: &mut Poses, s: &RenderState, h: &Humanoid) {
    let swing = s.swing();
    let Some(swing_right) = h.swing_right.filter(|_| swing > 0.0) else { return };
    let mut body_y = sin(swing.sqrt() * (PI * 2.0)) * 0.2;
    if !swing_right {
        body_y *= -1.0;
    }
    p.y_rot("body", body_y);
    let age_scale = s.age_scale();
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
    let bb = sin(swing * PI) * -(p.read("head").x_rot - 0.7) * 0.75;
    let attacking = p.get(arm(swing_right));
    attacking.x_rot -= aa * 1.2 + bb;
    attacking.y_rot += body_y * 2.0;
    attacking.z_rot += sin(swing * PI) * -0.4;
}

/// `AnimationUtils.animateZombieArms`.
fn zombie_arms(p: &mut Poses, s: &RenderState, h: &Humanoid, aggressive: bool) {
    if h.right == ArmPose::Spear || h.left == ArmPose::Spear {
        return;
    }
    let swing = s.swing();
    let raise = !s.baby() || !h.holding;
    let drop = if raise { -PI / if aggressive { 1.5 } else { 2.25 } } else { 0.0 };
    let ym = if raise { 1.0 } else { -1.0 } * sin(swing * PI);
    let xm = sin((1.0 - (1.0 - swing) * (1.0 - swing)) * PI);
    let x = drop + ym * 1.2 - xm * 0.4;
    let y = 0.1 - ym * 0.6;
    if h.right != ArmPose::ThrowTrident {
        p.x_rot("right_arm", x);
        p.y_rot("right_arm", if raise { -y } else { y });
        p.z_rot("right_arm", 0.0);
    }
    if h.left != ArmPose::ThrowTrident {
        p.x_rot("left_arm", x);
        p.y_rot("left_arm", if raise { y } else { -y });
        p.z_rot("left_arm", 0.0);
    }
}

/// `HumanoidModel.setupSwimAnimation`: the left arm eases the short way round, the right one
/// linearly.
fn swim(p: &mut Poses, pos: f32, h: &Humanoid) {
    let amount = h.swim;
    if !h.fall_flying {
        let x = p.read("head").x_rot;
        p.x_rot("head", rot_lerp_rad(amount, x, -FRAC_PI_4));
    }
    let swim_pos = pos % 26.0;
    let right_amount = if h.right != ArmPose::Spear && h.swing_right != Some(true) { amount } else { 0.0 };
    let left_amount = if h.left != ArmPose::Spear && h.swing_right != Some(false) { amount } else { 0.0 };
    let quadratic = |x: f32| -65.0 * x + x * x;
    // Targets (x, y, z) for the left and right arm.
    let targets = if h.using_item {
        None
    } else if swim_pos < 14.0 {
        let stroke = 1.870_796_4 * quadratic(swim_pos) / quadratic(14.0);
        Some(([0.0, PI, PI + stroke], [0.0, PI, PI - stroke]))
    } else if swim_pos < 22.0 {
        let t = (swim_pos - 14.0) / 8.0;
        Some(([PI / 2.0 * t, PI, 5.012_389 - 1.870_796_4 * t], [PI / 2.0 * t, PI, 1.270_796_3 + 1.870_796_4 * t]))
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
    p.x_rot("right_arm", -PI / 2.0);
    p.x_rot("left_arm", -PI / 2.0);
    p.get("right_arm").x_rot -= s2 * 1.2 - s1 * 0.4;
    p.get("left_arm").x_rot -= s2 * 1.2 - s1 * 0.4;
    bob_arms(p, s.age());
}
