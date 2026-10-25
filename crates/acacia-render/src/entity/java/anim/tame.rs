//! `WolfModel` (adult and baby), `LlamaModel`, `SquidModel` and `DolphinModel` (26.3 client).

use std::f32::consts::PI;

use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

/// `WolfRenderState.getBodyRollAngle`: the roll of a wolf shaking off water.
fn body_roll(s: &RenderState, offset: f32) -> f32 {
    let progress = ((s.num("shakeAnim") + offset) / 1.8).clamp(0.0, 1.0);
    sin(progress * PI) * sin(progress * PI * 11.0) * 0.15 * PI
}

pub(super) fn wolf(p: &mut Poses, s: &RenderState) {
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    p.y_rot("tail", if s.flag("isAngry") { 0.0 } else { cos(pos * 0.6662) * 1.4 * speed });
    let adult = !s.baby();
    if s.flag("isSitting") {
        wolf_sitting(p, s.age_scale(), adult);
    } else {
        p.x_rot("right_hind_leg", cos(pos * 0.6662) * 1.4 * speed);
        p.x_rot("left_hind_leg", cos(pos * 0.6662 + PI) * 1.4 * speed);
        p.x_rot("right_front_leg", cos(pos * 0.6662 + PI) * 1.4 * speed);
        p.x_rot("left_front_leg", cos(pos * 0.6662) * 1.4 * speed);
    }
    p.z_rot("body", body_roll(s, -0.16));
    let (head_roll, tail_roll) = (s.num("headRollAngle") + body_roll(s, 0.0), body_roll(s, -0.2));
    if adult {
        p.z_rot("real_head", head_roll);
        p.z_rot("upper_body", body_roll(s, -0.08));
        p.z_rot("real_tail", tail_roll);
    } else {
        p.z_rot("head", head_roll);
        p.z_rot("tail", tail_roll);
    }
    head(p, s);
    let tail = if s.has("tailAngle") { s.num("tailAngle") } else { pi_div(5.0) };
    p.x_rot("tail", tail);
}

fn wolf_sitting(p: &mut Poses, age: f32, adult: bool) {
    let body = p.get("body");
    body.y += 4.0 * age;
    body.z -= 2.0 * age;
    body.x_rot = pi_div(4.0);
    let tail = p.get("tail");
    tail.y += 9.0 * age;
    tail.z -= 2.0 * age;
    for leg in ["right_hind_leg", "left_hind_leg"] {
        let part = p.get(leg);
        part.y += 6.7 * age;
        part.z -= 5.0 * age;
        part.x_rot = (std::f64::consts::PI * 3.0 / 2.0) as f32;
    }
    for (leg, side) in [("right_front_leg", 1.0), ("left_front_leg", -1.0)] {
        let part = p.get(leg);
        part.x_rot = 5.811_947;
        part.x += side * 0.01 * age;
        part.y += 1.0 * age;
    }
    if adult {
        let upper = p.get("upper_body");
        upper.y += 2.0;
        upper.x_rot = (std::f64::consts::PI * 2.0 / 5.0) as f32;
        upper.y_rot = 0.0;
    } else {
        p.get("body").x_rot -= PI / 2.0;
    }
}

/// Legs walk like a quadruped's; the chests show when it carries them.
pub(super) fn llama(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    p.x_rot("right_hind_leg", cos(pos * 0.6662) * 1.4 * speed);
    p.x_rot("left_hind_leg", cos(pos * 0.6662 + PI) * 1.4 * speed);
    p.x_rot("right_front_leg", cos(pos * 0.6662 + PI) * 1.4 * speed);
    p.x_rot("left_front_leg", cos(pos * 0.6662) * 1.4 * speed);
    let chest = s.flag("hasChest");
    p.set_visible("right_chest", chest);
    p.set_visible("left_chest", chest);
}

pub(super) fn squid(p: &mut Poses, s: &RenderState) {
    for i in 0..8 {
        p.x_rot(&format!("tentacle{i}"), s.num("tentacleAngle"));
    }
}

/// The body follows the look; swimming beats the tail.
pub(super) fn dolphin(p: &mut Poses, s: &RenderState) {
    let mut x = s.x_rot() * pi_div(180.0);
    p.y_rot("body", s.y_rot() * pi_div(180.0));
    if s.flag("isMoving") {
        let beat = cos(s.age() * 0.3);
        x += -0.05 - 0.05 * beat;
        p.x_rot("tail", -0.1 * beat);
        p.x_rot("tail_fin", -0.2 * beat);
    }
    p.x_rot("body", x);
}
