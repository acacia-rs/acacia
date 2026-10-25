//! `AdultFelineModel` and `BabyFelineModel` (26.3 client): cats and ocelots crouching,
//! sprinting, sitting and lying down.

use std::f32::consts::PI;

use super::super::mth::{cos, pi_div, rot_lerp};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

const TAIL_UP: f32 = 1.727_876_1;

pub(super) fn feline(p: &mut Poses, s: &RenderState) {
    let (age, baby) = (s.age_scale(), s.baby());
    let (crouching, sprinting, sitting) = (s.flag("isCrouching"), s.flag("isSprinting"), s.flag("isSitting"));
    if crouching {
        p.get("body").y += 1.0 * age;
        p.get("head").y += 2.0 * age;
        p.get("tail1").y += 1.0 * age;
        p.get("tail2").y += -4.0 * age;
        p.get("tail2").z += 2.0 * age;
        p.x_rot("tail1", PI / 2.0);
        p.x_rot("tail2", PI / 2.0);
    } else if sprinting {
        let y = p.read("tail1").y;
        p.get("tail2").y = y;
        p.get("tail2").z += 2.0 * age;
        p.x_rot("tail1", PI / 2.0);
        p.x_rot("tail2", PI / 2.0);
    }
    head(p, s);
    if !sitting {
        if !baby {
            p.x_rot("body", PI / 2.0);
        }
        let (speed, pos) = (s.walk_speed(), s.walk_pos());
        let (right_hind, lag, tail_swing) = if sprinting {
            (0.3, 0.3, pi_div(10.0))
        } else {
            (PI, 0.0, if crouching { 0.471_238_94 } else { pi_div(4.0) })
        };
        p.x_rot("left_hind_leg", cos(pos * 0.6662) * speed);
        p.x_rot("right_hind_leg", cos(pos * 0.6662 + right_hind) * speed);
        p.x_rot("left_front_leg", cos(pos * 0.6662 + PI + lag) * speed);
        p.x_rot("right_front_leg", cos(pos * 0.6662 + if sprinting { PI } else { 0.0 }) * speed);
        p.x_rot("tail2", TAIL_UP + tail_swing * cos(pos) * speed);
    } else if baby {
        baby_sitting(p);
    } else {
        adult_sitting(p, age);
    }
    let lie = s.num("lieDownAmount");
    if lie > 0.0 {
        if baby {
            baby_lying(p, lie, s.num("lieDownAmountTail"));
        } else {
            adult_lying(p, lie, s.num("lieDownAmountTail"), age);
        }
    }
    let relax = s.num("relaxStateOneAmount");
    if relax > 0.0 {
        let x = p.read("head").x_rot;
        p.x_rot("head", rot_lerp(relax, x, -0.581_776_44));
    }
}

/// Moves `name` by (x, y, z) and sets its x rotation, when given.
fn shift(p: &mut Poses, name: &str, by: [f32; 3], x_rot: Option<f32>) {
    let part = p.get(name);
    part.x += by[0];
    part.y += by[1];
    part.z += by[2];
    if let Some(x) = x_rot {
        part.x_rot = x;
    }
}

fn adult_sitting(p: &mut Poses, a: f32) {
    shift(p, "body", [0.0, -4.0 * a, 5.0 * a], Some(pi_div(4.0)));
    shift(p, "head", [0.0, -3.3 * a, 1.0 * a], None);
    shift(p, "tail1", [0.0, 8.0 * a, -2.0 * a], Some(TAIL_UP));
    shift(p, "tail2", [0.0, 2.0 * a, -0.8 * a], Some(2.670_354));
    shift(p, "left_front_leg", [0.0, 2.0 * a, -2.0 * a], Some(pi_div(-20.0)));
    shift(p, "right_front_leg", [0.0, 2.0 * a, -2.0 * a], Some(pi_div(-20.0)));
    shift(p, "left_hind_leg", [0.0, 3.0 * a, -4.0 * a], Some(-PI / 2.0));
    shift(p, "right_hind_leg", [0.0, 3.0 * a, -4.0 * a], Some(-PI / 2.0));
}

fn baby_sitting(p: &mut Poses) {
    p.get("body").x_rot += -0.436_332_32;
    shift(p, "body", [0.0, 1.25, 0.0], None);
    shift(p, "head", [0.0, 0.0, 0.75], None);
    p.get("tail1").x_rot += 0.545_415_4;
    shift(p, "tail1", [0.0, 4.0, -0.9], None);
    shift(p, "left_hind_leg", [0.0, 0.0, -0.9], None);
    shift(p, "right_hind_leg", [0.0, 0.0, -0.9], None);
}

fn adult_lying(p: &mut Poses, lie: f32, tail: f32, a: f32) {
    let head = p.get("head");
    head.z_rot = rot_lerp(lie, head.z_rot, -1.270_796_3);
    head.y_rot = rot_lerp(lie, head.y_rot, 1.270_796_3);
    p.x_rot("left_front_leg", -1.270_796_3);
    shift(p, "right_front_leg", [a, 0.0, 0.0], Some(-0.470_796_35));
    p.z_rot("right_front_leg", -0.2);
    p.x_rot("left_hind_leg", -0.4);
    shift(p, "right_hind_leg", [0.8 * a, 2.0 * a, 0.0], Some(0.5));
    p.z_rot("right_hind_leg", -0.5);
    let x = p.read("tail1").x_rot;
    p.x_rot("tail1", rot_lerp(tail, x, 0.8));
    let x = p.read("tail2").x_rot;
    p.x_rot("tail2", rot_lerp(tail, x, -0.4));
}

fn baby_lying(p: &mut Poses, lie: f32, tail: f32) {
    p.get("body").x += 1.0;
    let head = p.get("head");
    head.x_rot = rot_lerp(lie, head.x_rot, pi_div(18.0));
    head.z_rot = rot_lerp(lie, head.z_rot, (-std::f64::consts::PI * 5.0 / 12.0) as f32);
    shift(p, "head", [1.5, 0.75, -0.5], None);
    shift(p, "right_front_leg", [3.5, -0.5, 0.0], Some(pi_div(-4.0)));
    shift(p, "left_front_leg", [1.5, -1.0, -2.0], Some(-PI / 2.0));
    shift(p, "right_hind_leg", [2.5, -0.25, 0.5], Some((std::f64::consts::PI * 2.0 / 9.0) as f32));
    p.y_rot("right_hind_leg", pi_div(9.0));
    p.z_rot("right_hind_leg", pi_div(-9.0));
    shift(p, "left_hind_leg", [1.5, 0.0, -1.0], None);
    let t = p.get("tail1");
    t.x_rot += rot_lerp(tail, t.x_rot, pi_div(-6.0));
    t.y_rot += rot_lerp(tail, t.y_rot, 0.0);
    t.z_rot += rot_lerp(tail, t.z_rot, pi_div(-18.0));
    shift(p, "tail1", [1.0, 0.5, -0.25], None);
}
