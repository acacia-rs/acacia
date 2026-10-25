//! `StriderModel` (adult and baby), `RavagerModel`, `ShulkerModel` and `WitherBossModel`
//! (26.3 client).

use std::f32::consts::PI;

use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;
use super::simple::triangle_wave;

/// `StriderModel.animateBristle`: each bristle sways with the walk and in the heat.
fn bristles(p: &mut Poses, age: f32, flow: f32, parts: [&str; 3], about_z: bool) {
    let sway = [flow * 0.6, flow * 1.2, flow * 1.3];
    let idle = [0.1 * sin(age * 0.4), 0.1 * sin(age * 0.2), 0.05 * sin(age * -0.4)];
    for (part, (a, b)) in parts.into_iter().zip(sway.into_iter().zip(idle)) {
        let pose = p.get(part);
        let rot = if about_z { &mut pose.z_rot } else { &mut pose.x_rot };
        *rot += a;
        *rot += b;
    }
}

pub(super) fn strider(p: &mut Poses, s: &RenderState) {
    let (pos, speed, age) = (s.walk_pos(), s.walk_speed().min(0.25), s.age());
    let (x, y) = if s.flag("isRidden") { (0.0, 0.0) } else { (s.x_rot() * pi_div(180.0), s.y_rot() * pi_div(180.0)) };
    let body = p.get("body");
    (body.x_rot, body.y_rot) = (x, y);
    body.z_rot = 0.1 * sin(pos * 1.5) * 4.0 * speed;
    let step = pos * 1.5 * 0.5;
    p.x_rot("left_leg", sin(step) * 2.0 * speed);
    p.x_rot("right_leg", sin(step + PI) * 2.0 * speed);
    p.z_rot("left_leg", pi_div(18.0) * cos(step) * speed);
    p.z_rot("right_leg", pi_div(18.0) * cos(step + PI) * speed);
    let flow = cos(pos * 1.5 + PI) * speed;
    let (body_y, body_bob, leg_y) = if s.baby() { (17.25, 1.0, 20.0) } else { (2.0, 2.0, 8.0) };
    if s.baby() {
        p.get("body").y = body_y - body_bob * cos(pos * 1.5) * 2.0 * speed;
        p.get("left_leg").y = leg_y + 2.0 * sin(step + PI) * 2.0 * speed;
        p.get("right_leg").y = leg_y + 2.0 * sin(step) * 2.0 * speed;
        bristles(p, age, flow, ["bristle2", "bristle1", "bristle0"], false);
        return;
    }
    for (part, z) in [
        ("right_bottom_bristle", -1.221_730_5),
        ("right_middle_bristle", -1.134_464),
        ("right_top_bristle", -0.872_664_63),
        ("left_top_bristle", 0.872_664_63),
        ("left_middle_bristle", 1.134_464),
        ("left_bottom_bristle", 1.221_730_5),
    ] {
        p.z_rot(part, z);
    }
    bristles(p, age, flow, ["right_top_bristle", "right_middle_bristle", "right_bottom_bristle"], true);
    bristles(p, age, flow, ["left_top_bristle", "left_middle_bristle", "left_bottom_bristle"], true);
    p.get("body").y = body_y - body_bob * cos(pos * 1.5) * 2.0 * speed;
    p.get("left_leg").y = leg_y + 2.0 * sin(step + PI) * 2.0 * speed;
    p.get("right_leg").y = leg_y + 2.0 * sin(step) * 2.0 * speed;
}

/// The neck lunges in an attack, sways when stunned; the mouth opens to roar.
pub(super) fn ravager(p: &mut Poses, s: &RenderState) {
    let (stunned, attack) = (s.num("stunnedTicksRemaining"), s.num("attackTicksRemaining"));
    let neck_x = p.read("neck").x_rot;
    if attack > 0.0 {
        let scaled = (1.0 + triangle_wave(attack, 10.0)) * 0.5;
        let reach = scaled * scaled * scaled * 12.0;
        let neck = p.get("neck");
        neck.z = -6.5 + reach;
        neck.y = -7.0 - reach * sin(neck_x);
        let mouth = if attack > 5.0 { sin((-4.0 + attack) / 4.0) * PI * 0.4 } else { pi_div(20.0) * sin(PI * attack / 10.0) };
        p.x_rot("mouth", mouth);
    } else {
        let neck = p.get("neck");
        (neck.x, neck.y, neck.z) = (0.0, -7.0 - -1.0 * sin(neck_x), 5.5);
        let is_stunned = stunned > 0.0;
        neck.x_rot = if is_stunned { 0.219_911_49 } else { 0.0 };
        p.x_rot("mouth", PI * if is_stunned { 0.05 } else { 0.01 });
        let roar = s.num("roarAnimation");
        if is_stunned {
            p.get("neck").x = ((f64::from(stunned) / 40.0 * 10.0).sin() as f32) * 3.0;
        } else if roar > 0.0 {
            p.x_rot("mouth", (PI / 2.0) * sin(roar * PI * 0.25));
        }
    }
    head(p, s);
    let (pos, legs) = (s.walk_pos(), 0.4 * s.walk_speed());
    p.x_rot("right_hind_leg", cos(pos * 0.6662) * legs);
    p.x_rot("left_hind_leg", cos(pos * 0.6662 + PI) * legs);
    p.x_rot("right_front_leg", cos(pos * 0.6662 + PI) * legs);
    p.x_rot("left_front_leg", cos(pos * 0.6662) * legs);
}

/// The lid rises with the peek and twists; the head looks out.
pub(super) fn shulker(p: &mut Poses, s: &RenderState) {
    let peek = s.num("peekAmount");
    let bs = (0.5 + peek) * PI;
    let q = -1.0 + sin(bs);
    let extra = if bs > PI { sin(s.age() * 0.1) * 0.7 } else { 0.0 };
    let lid = p.get("lid");
    (lid.x, lid.y, lid.z) = (0.0, 16.0 + sin(bs) * 8.0 + extra, 0.0);
    lid.y_rot = if peek > 0.3 { q * q * q * q * PI * 0.125 } else { 0.0 };
    p.x_rot("head", s.x_rot() * pi_div(180.0));
    p.y_rot("head", (s.num("yHeadRot") - 180.0 - s.num("yBodyRot")) * pi_div(180.0));
}

/// The side heads look on their own; the ribcage breathes and the tail follows it.
pub(super) fn wither(p: &mut Poses, s: &RenderState) {
    for (i, part) in ["right_head", "left_head"].into_iter().enumerate() {
        p.y_rot(part, (s.num(&format!("yHeadRots{i}")) - s.num("bodyRot")) * pi_div(180.0));
        p.x_rot(part, s.num(&format!("xHeadRots{i}")) * pi_div(180.0));
    }
    let anim = cos(s.age() * 0.1);
    let rib = (0.065 + 0.05 * anim) * PI;
    p.x_rot("ribcage", rib);
    let tail = p.get("tail");
    (tail.x, tail.y, tail.z) = (-2.0, 6.9 + cos(rib) * 10.0, -0.5 + sin(rib) * 10.0);
    tail.x_rot = (0.265 + 0.1 * anim) * PI;
    p.y_rot("center_head", s.y_rot() * pi_div(180.0));
    p.x_rot("center_head", s.x_rot() * pi_div(180.0));
}
