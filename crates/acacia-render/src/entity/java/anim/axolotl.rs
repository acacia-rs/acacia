//! `AdultAxolotlModel` (26.3 client; the baby's is keyframed): swimming, hovering, crawling,
//! lying still and playing dead, blended by factors, the right legs mirroring the left.

use std::f32::consts::PI;

use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;

const QUIET: f32 = 1.0e-5;

fn add(p: &mut Poses, part: &str, [x, y, z]: [f32; 3]) {
    let pose = p.get(part);
    pose.x_rot += x;
    pose.y_rot += y;
    pose.z_rot += z;
}

/// `gills`: top x, left y (the right one turns the other way).
fn gills(p: &mut Poses, top: f32, sides: f32) {
    p.get("top_gills").x_rot += top;
    p.get("left_gills").y_rot += sides;
    p.get("right_gills").y_rot -= sides;
}

pub(super) fn axolotl(p: &mut Poses, s: &RenderState) {
    let water = if s.has("inWaterFactor") { s.num("inWaterFactor") } else { 1.0 };
    let (dead, ground, moving) = (s.num("playingDeadFactor"), s.num("onGroundFactor"), s.num("movingFactor"));
    let still = 1.0 - moving;
    let mirrored = 1.0 - ground.min(moving);
    let age = s.age();
    p.get("body").y_rot += s.y_rot() * pi_div(180.0);
    swimming(p, age, s.x_rot(), moving.min(water));
    hovering(p, age, still.min(water));
    crawling(p, age, moving.min(ground));
    lying(p, age, still.min(ground));
    if dead > QUIET {
        add(p, "left_hind_leg", [1.413_716_7 * dead, 1.099_557_4 * dead, pi_div(4.0) * dead]);
        add(p, "left_front_leg", [pi_div(4.0) * dead, 2.042_035 * dead, 0.0]);
        add(p, "body", [-0.15 * dead, 0.0, 0.35 * dead]);
    }
    if mirrored > QUIET {
        for (right, left) in [("right_hind_leg", "left_hind_leg"), ("right_front_leg", "left_front_leg")] {
            let from = p.read(left);
            add(p, right, [from.x_rot * mirrored, -from.y_rot * mirrored, -from.z_rot * mirrored]);
        }
    }
}

fn swimming(p: &mut Poses, age: f32, x_rot: f32, f: f32) {
    if f <= QUIET {
        return;
    }
    let speed = age * 0.33;
    let (sine, cosine) = (sin(speed), cos(speed));
    let sway = 0.13 * sine;
    p.get("body").x_rot += (x_rot * pi_div(180.0) + sway) * f;
    p.get("head").x_rot -= sway * 1.8 * f;
    p.get("body").y -= 0.45 * cosine * f;
    gills(p, (-0.5 * sine - 0.8) * f, (0.3 * sine + 0.9) * f);
    p.get("tail").y_rot += 0.3 * cos(speed * 0.9) * f;
    add(p, "left_hind_leg", [1.884_955_8 * f, -0.4 * sine * f, PI / 2.0 * f]);
    add(p, "left_front_leg", [1.884_955_8 * f, (-0.2 * cosine - 0.1) * f, PI / 2.0 * f]);
}

fn hovering(p: &mut Poses, age: f32, f: f32) {
    if f <= QUIET {
        return;
    }
    let speed = age * 0.075;
    let (cosine, sine) = (cos(speed), sin(speed) * 0.15);
    let body_x = (-0.15 + 0.075 * cosine) * f;
    p.get("body").x_rot += body_x;
    p.get("body").y -= sine * f;
    p.get("head").x_rot -= body_x;
    p.get("top_gills").x_rot += 0.2 * cosine * f;
    let side = (-0.3 * cosine - 0.19) * f;
    p.get("left_gills").y_rot += side;
    p.get("right_gills").y_rot -= side;
    add(p, "left_hind_leg", [((std::f64::consts::PI * 3.0 / 4.0) as f32 - cosine * 0.11) * f, 0.471_238_94 * f, 1.727_876_1 * f]);
    add(p, "left_front_leg", [(pi_div(4.0) - cosine * 0.2) * f, 2.042_035 * f, 0.0]);
    p.get("tail").y_rot += 0.5 * cosine * f;
}

fn crawling(p: &mut Poses, age: f32, f: f32) {
    if f <= QUIET {
        return;
    }
    let speed = age * 0.11;
    let cosine = cos(speed);
    let hind_sway = (cosine * cosine - 2.0 * cosine) / 5.0;
    let front_sway = 0.7 * cosine;
    let turn = 0.09 * cosine * f;
    p.get("head").y_rot += turn;
    p.get("tail").y_rot += turn;
    let gill = (0.6 - 0.08 * (cosine * cosine + 2.0 * sin(speed))) * f;
    gills(p, gill, -gill);
    let (hind_x, front_x) = (0.942_477_9 * f, 1.099_557_4 * f);
    add(p, "left_hind_leg", [hind_x, (1.5 - hind_sway) * f, -0.1 * f]);
    add(p, "left_front_leg", [front_x, (PI / 2.0 - front_sway) * f, 0.0]);
    add(p, "right_hind_leg", [hind_x, (-1.0 - hind_sway) * f, 0.0]);
    add(p, "right_front_leg", [front_x, (-PI / 2.0 - front_sway) * f, 0.0]);
}

fn lying(p: &mut Poses, age: f32, f: f32) {
    if f <= QUIET {
        return;
    }
    let speed = age * 0.09;
    let (sine, cosine) = (sin(speed), cos(speed));
    let movement = sine * sine - 2.0 * sine;
    let movement2 = cosine * cosine - 3.0 * sine;
    p.get("head").x_rot += -0.09 * movement * f;
    p.get("head").z_rot += -0.2 * f;
    p.get("tail").y_rot += (-0.1 + 0.1 * movement) * f;
    let gill = (0.6 + 0.05 * movement2) * f;
    gills(p, gill, -gill);
    add(p, "left_hind_leg", [1.1 * f, 1.0 * f, 0.0]);
    add(p, "left_front_leg", [0.8 * f, 2.3 * f, -0.5 * f]);
}
