//! Expectations carried over from Horizon's own tests of these ports (`mobs/anim.rs`), plus the
//! family table.

use std::f32::consts::{FRAC_PI_4, FRAC_PI_6, PI};

use super::super::model::{Part, PartPose};
use super::super::mth::cos;
use super::super::poses::Poses;
use super::super::state::{Field, RenderState};
use super::*;

fn model(names: &[&str]) -> Part {
    let part = |name: &str| Part { name: name.into(), pose: PartPose::default(), polygons: vec![], children: vec![] };
    Part { children: names.iter().map(|n| part(n)).collect(), ..part("root") }
}

fn humanoid_poses() -> Poses {
    Poses::new(&model(&["head", "body", "right_arm", "left_arm", "right_leg", "left_leg"]))
}

fn state(fields: &[(&str, Field)]) -> RenderState {
    let mut s = RenderState::default();
    for (name, value) in fields {
        s.set(name, value.clone());
    }
    s
}

#[test]
fn blocking_arm_follows_the_head_within_limits() {
    for right in [false, true] {
        for (pitch, yaw, expected_pitch, expected_yaw) in
            [(0.0, 0.0, -0.742_477_9, 0.0), (2.0, 2.0, -0.306_145_58, FRAC_PI_6), (-2.0, -2.0, -2.138_741_3, -FRAC_PI_6)]
        {
            let mut p = humanoid_poses();
            let arm = if right { "right_arm" } else { "left_arm" };
            p.x_rot("head", pitch);
            p.y_rot("head", yaw);
            p.x_rot(arm, 0.4);
            arms::pose_arm(&mut p, ArmPose::Block, right, false, (0.0, 0.0));
            let pose = p.read(arm);
            assert!((pose.x_rot - expected_pitch).abs() < 1e-6);
            let side = if right { -FRAC_PI_6 } else { FRAC_PI_6 };
            assert!((pose.y_rot - (side + expected_yaw)).abs() < 1e-6);
            assert_eq!(p.read(if right { "left_arm" } else { "right_arm" }).x_rot, 0.0);
        }
    }
}

#[test]
fn gliding_slows_the_limbs_and_tips_the_head() {
    let walking = state(&[("walkAnimationSpeed", Field::Num(1.0))]);
    let gliding = walking.clone().with("isFallFlying", Field::Flag(true)).with("speedValue", Field::Num(8.0));
    let (mut ground, mut flight) = (humanoid_poses(), humanoid_poses());
    setup(Family::Skeleton, &walking, &mut ground);
    setup(Family::Skeleton, &gliding, &mut flight);
    assert_eq!(flight.read("head").x_rot, -FRAC_PI_4);
    assert_eq!(flight.read("right_leg").x_rot, ground.read("right_leg").x_rot / 8.0);
}

#[test]
fn zombie_arms_leave_a_raised_trident_alone() {
    let s = state(&[
        ("walkAnimationSpeed", Field::Num(1.0)),
        ("rightArmPose", Field::Text("THROW_TRIDENT".into())),
        ("leftArmPose", Field::Text("ITEM".into())),
        ("isAggressive", Field::Flag(true)),
    ]);
    let mut p = humanoid_poses();
    setup(Family::Zombie, &s, &mut p);
    let still = cos(PI) * 2.0 * 0.5;
    // bobModelPart at age 0 adds nothing to x.
    assert_eq!(p.read("right_arm").x_rot, still * 0.5 - PI);
    assert_eq!(p.read("left_arm").x_rot, -PI / 1.5);
}

#[test]
fn swimming_strokes_the_arms_and_kicks() {
    let s = state(&[("walkAnimationPos", Field::Num(7.0)), ("walkAnimationSpeed", Field::Num(1.0)), ("xRot", Field::Num(30.0)), ("swimAmount", Field::Num(1.0))]);
    let same_angle = |a: f32, b: f32| ((a - b + PI).rem_euclid(PI * 2.0) - PI).abs() < 1e-5;
    let mut p = humanoid_poses();
    setup(Family::Skeleton, &s, &mut p);
    let stroke = 1.870_796_4 * (-65.0 * 7.0 + 49.0) / (-65.0 * 14.0 + 196.0);
    assert_eq!(p.read("right_arm").z_rot, PI - stroke);
    assert!(same_angle(p.read("left_arm").z_rot, PI + stroke));
    assert_eq!(p.read("right_arm").y_rot, PI);
    assert!(same_angle(p.read("head").x_rot, -FRAC_PI_4));
    assert!(same_angle(p.read("right_leg").x_rot, 0.3 * cos(7.0 * 0.333_333_34)));
    let mut using = humanoid_poses();
    setup(Family::Skeleton, &s.clone().with("isUsingItem", Field::Flag(true)), &mut using);
    assert_eq!(using.read("right_arm").y_rot, 0.0);
}

#[test]
fn sheep_eating_lowers_the_head() {
    assert_eq!(sheep_eat_scales(0, 0.0, 30.0), (0.0, 30.0f32.to_radians()));
    assert_eq!(sheep_eat_scales(2, 0.0, 0.0), (0.5, crate::entity::java::mth::pi_div(5.0)));
    assert_eq!(sheep_eat_scales(39, 0.0, 0.0).0, 0.25);
    let mut p = Poses::new(&model(&["head", "right_hind_leg", "left_hind_leg", "right_front_leg", "left_front_leg"]));
    setup(Family::Sheep, &state(&[("headEatPositionScale", Field::Num(1.0)), ("headEatAngleScale", Field::Num(0.6))]), &mut p);
    assert_eq!((p.read("head").y, p.read("head").x_rot), (9.0, 0.6));
}

#[test]
fn families_by_layer() {
    assert_eq!(Family::of_layer("minecraft:cow#main"), Some(Family::Quadruped));
    assert_eq!(Family::of_layer("minecraft:zombie_baby#main"), Some(Family::Zombie));
    assert_eq!(Family::of_layer("minecraft:chicken_baby#main"), Some(Family::BabyChicken));
    assert_eq!(Family::of_layer("minecraft:sheep#wool"), None);
    assert_eq!(Family::of_layer("minecraft:warden#main"), None);
}
