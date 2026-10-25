//! `BeeModel` (adult and baby), `ParrotModel` and `AdultFoxModel` (26.3 client).

use std::f32::consts::PI;

use super::super::mth::{cos, pi_div, rot_lerp_rad, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

/// Wings beat and legs tuck in flight; a calm bee bobs; a rolling one tips over.
pub(super) fn bee(p: &mut Poses, s: &RenderState) {
    let age = s.age();
    p.set_visible("stinger", !s.has("hasStinger") || s.flag("hasStinger"));
    let flying = !s.flag("isOnGround");
    if flying {
        let beat = cos(age * 120.321_13 * pi_div(180.0)) * PI * 0.15;
        let x = p.read("right_wing").x_rot;
        p.y_rot("right_wing", 0.0);
        p.z_rot("right_wing", beat);
        let left = p.get("left_wing");
        (left.x_rot, left.y_rot, left.z_rot) = (x, 0.0, -beat);
        for legs in ["front_legs", "middle_legs", "back_legs"] {
            p.x_rot(legs, pi_div(4.0));
        }
    }
    if !s.flag("isAngry") && flying {
        let speed = cos(age * 0.18);
        p.x_rot("bone", 0.1 + speed * PI * 0.025);
        p.get("bone").y -= cos(age * 0.18) * 0.9;
        p.x_rot("front_legs", -speed * PI * 0.1 + pi_div(8.0));
        p.x_rot("back_legs", -speed * PI * 0.05 + pi_div(4.0));
        if !s.baby() {
            p.x_rot("left_antenna", speed * PI * 0.03);
            p.x_rot("right_antenna", speed * PI * 0.03);
        }
    }
    let roll = s.num("rollAmount");
    if roll > 0.0 {
        let x = p.read("bone").x_rot;
        p.x_rot("bone", rot_lerp_rad(roll, x, 3.091_592_8));
    }
}

const PARROT_BODY: [&str; 7] = ["head", "tail", "body", "left_wing", "right_wing", "left_leg", "right_leg"];

/// `ParrotModel.prepare` then the pose's own motion (Java's switch falls through).
pub(super) fn parrot(p: &mut Poses, s: &RenderState) {
    let pose = s.text("pose").unwrap_or("FLYING");
    match pose {
        "FLYING" => {
            p.get("left_leg").x_rot += (std::f64::consts::PI * 2.0 / 9.0) as f32;
            p.get("right_leg").x_rot += (std::f64::consts::PI * 2.0 / 9.0) as f32;
        }
        "SITTING" => {
            for part in PARROT_BODY {
                p.get(part).y += 1.9;
            }
            p.get("tail").x_rot += pi_div(6.0);
            p.z_rot("left_wing", -0.0873);
            p.z_rot("right_wing", 0.0873);
            p.get("left_leg").x_rot += PI / 2.0;
            p.get("right_leg").x_rot += PI / 2.0;
        }
        "PARTY" => {
            p.z_rot("left_leg", pi_div(-9.0));
            p.z_rot("right_leg", pi_div(9.0));
        }
        _ => {}
    }
    head(p, s);
    let (pos, speed, flap) = (s.walk_pos(), s.walk_speed(), s.num("flapAngle"));
    match pose {
        "SITTING" => {}
        "PARTY" => {
            let (x, y) = (cos(s.age()), sin(s.age()));
            let head = p.get("head");
            (head.x_rot, head.y_rot, head.z_rot) = (0.0, 0.0, sin(s.age()) * 0.4);
            p.z_rot("left_wing", -0.0873 - flap);
            p.z_rot("right_wing", 0.0873 + flap);
            for part in ["head", "body", "left_wing", "right_wing", "tail"] {
                let part = p.get(part);
                part.x += x;
                part.y += y;
            }
        }
        _ => {
            if pose == "STANDING" {
                p.get("left_leg").x_rot += cos(pos * 0.6662) * 1.4 * speed;
                p.get("right_leg").x_rot += cos(pos * 0.6662 + PI) * 1.4 * speed;
            }
            let bob = flap * 0.3;
            p.get("tail").x_rot += cos(pos * 0.6662) * 0.3 * speed;
            for part in PARROT_BODY {
                p.get(part).y += bob;
            }
            p.z_rot("left_wing", -0.0873 - flap);
            p.z_rot("right_wing", 0.0873 + flap);
        }
    }
}

/// `AdultFoxModel` (the baby's is keyframed): walking, crouching, sleeping, sitting, pouncing.
// TODO: faceplanting; Java keeps its leg motion in the model between frames.
pub(super) fn fox(p: &mut Poses, s: &RenderState) {
    let (pos, speed, age) = (s.walk_pos(), s.walk_speed(), s.age());
    let legs = ["right_hind_leg", "left_hind_leg", "right_front_leg", "left_front_leg"];
    p.z_rot("head", s.num("headRollAngle"));
    for leg in legs {
        p.set_visible(leg, true);
    }
    p.x_rot("right_hind_leg", cos(pos * 0.6662) * 1.4 * speed);
    p.x_rot("left_hind_leg", cos(pos * 0.6662 + PI) * 1.4 * speed);
    p.x_rot("right_front_leg", cos(pos * 0.6662 + PI) * 1.4 * speed);
    p.x_rot("left_front_leg", cos(pos * 0.6662) * 1.4 * speed);
    let (crouching, sleeping, crouch) = (s.flag("isCrouching"), s.flag("isSleeping"), s.num("crouchAmount"));
    if crouching {
        p.get("body").x_rot += 0.104_719_76;
        p.get("head").y += crouch * s.age_scale();
        let wiggle = cos(age) * 0.05;
        p.y_rot("body", wiggle);
        p.z_rot("right_hind_leg", wiggle);
        p.z_rot("left_hind_leg", wiggle);
        p.z_rot("right_front_leg", wiggle / 2.0);
        p.z_rot("left_front_leg", wiggle / 2.0);
        p.get("body").y += crouch;
    } else if sleeping {
        for leg in legs {
            p.set_visible(leg, false);
        }
        let body = p.get("body");
        body.z_rot = -PI / 2.0;
        body.y += 5.0;
        p.x_rot("tail", (-std::f64::consts::PI * 5.0 / 6.0) as f32);
        let head = p.get("head");
        head.x += 2.0;
        head.y += 2.99;
        head.y_rot = (-std::f64::consts::PI * 2.0 / 3.0) as f32;
        head.z_rot = 0.0;
    } else if s.flag("isSitting") {
        fox_sitting(p);
    }
    if s.flag("isPouncing") {
        let half = crouch / 2.0;
        p.get("body").y -= half;
        p.get("head").y -= half;
    }
    if !sleeping && !s.flag("isFaceplanted") && !crouching {
        head(p, s);
    }
    if sleeping {
        let head = p.get("head");
        (head.x_rot, head.y_rot, head.z_rot) = (0.0, (-std::f64::consts::PI * 2.0 / 3.0) as f32, cos(age * 0.027) / 22.0);
    }
}

fn fox_sitting(p: &mut Poses) {
    let head = p.get("head");
    (head.x_rot, head.y_rot) = (0.0, 0.0);
    head.y -= 6.5;
    head.z += 2.75;
    let body = p.get("body");
    body.x_rot = pi_div(6.0);
    body.y -= 7.0;
    body.z += 3.0;
    let tail = p.get("tail");
    tail.x_rot = pi_div(4.0);
    tail.z -= 1.0;
    p.x_rot("right_front_leg", pi_div(-12.0));
    p.x_rot("left_front_leg", pi_div(-12.0));
    for leg in ["right_hind_leg", "left_hind_leg"] {
        let part = p.get(leg);
        part.x_rot = (-std::f64::consts::PI * 5.0 / 12.0) as f32;
        part.y += 4.0;
        part.z -= 0.25;
    }
}
