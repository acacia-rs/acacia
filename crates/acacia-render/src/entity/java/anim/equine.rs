//! `AbstractEquineModel` and its subclasses (26.3 client): horses, undead horses, donkeys and
//! mules, adult and baby, walking, grazing, rearing and swishing their tails.

use std::f32::consts::PI;

use super::super::mth::{cos, lerp, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;

/// The overridable getters: stand angle, standing y and z offsets, standing x rotation, tail.
struct Shape {
    stand_angle: f32,
    stand_y: f32,
    stand_z: f32,
    stand_x_rot: f32,
    tail_x_rot: f32,
}

fn shape(donkey: bool, baby: bool) -> Shape {
    match (donkey, baby) {
        (true, true) => Shape { stand_angle: pi_div(3.0), stand_y: 1.0, stand_z: 0.5, stand_x_rot: 0.0, tail_x_rot: pi_div(-4.0) },
        (false, true) => Shape { stand_angle: pi_div(12.0), stand_y: 4.0, stand_z: 0.0, stand_x_rot: pi_div(-3.0), tail_x_rot: -PI / 2.0 },
        _ => Shape { stand_angle: pi_div(12.0), stand_y: 12.0, stand_z: 4.0, stand_x_rot: pi_div(-3.0), tail_x_rot: 0.0 },
    }
}

/// `(1 - max(standing, eating)) * (π/6 + head pitch + feeding bob)`.
fn base_head_angle(s: &RenderState, head_x: f32) -> f32 {
    let (eating, standing) = (s.num("eatAnimation"), s.num("standAnimation"));
    (1.0 - standing.max(eating)) * (pi_div(6.0) + head_x + s.num("feedingAnimation") * sin(s.age()) * 0.05)
}

pub(super) fn equine(p: &mut Poses, s: &RenderState, donkey: bool) {
    let baby = s.baby();
    let g = shape(donkey, baby);
    let clamped_y = s.y_rot().clamp(-20.0, 20.0);
    let (speed, pos, age) = (s.walk_speed(), s.walk_pos(), s.age());
    let mut head_x = s.x_rot() * pi_div(180.0);
    if speed > 0.2 {
        head_x += cos(pos * 0.8) * 0.15 * speed;
    }
    let (eating, standing) = (s.num("eatAnimation"), s.num("standAnimation"));
    let i_standing = 1.0 - standing;
    let head_y = clamped_y * pi_div(180.0);
    let water = if s.flag("isInWater") { 0.2 } else { 1.0 };
    let leg_anim = cos(water * pos * 0.6662 + PI);
    let leg_x = leg_anim * 0.8 * speed;
    let base = base_head_angle(s, head_x);
    let parts = p.get("head_parts");
    parts.x_rot = standing * (pi_div(12.0) + head_x) + eating * (2.181_661_6 + sin(age) * 0.05) + base;
    parts.y_rot = standing * clamped_y * pi_div(180.0) + (1.0 - standing.max(eating)) * head_y;
    match (donkey, baby) {
        (true, true) => {
            parts.y = lerp(eating, parts.y, -1.2);
            parts.z = lerp(standing, parts.z, -3.6);
        }
        (false, true) => {
            parts.y += lerp(eating, lerp(standing, 0.0, -2.0), 2.0);
            parts.z = lerp(standing, parts.z, -4.0);
        }
        _ => {
            parts.y += lerp(eating, lerp(standing, 0.0, -8.0), 7.0);
            parts.z = lerp(standing, parts.z, -4.0);
        }
    }
    let body = p.get("body");
    body.x_rot = standing * pi_div(-4.0) + i_standing * body.x_rot;
    let front = p.get("left_front_leg");
    front.y -= g.stand_y * standing;
    front.z += g.stand_z * standing;
    let (y, z) = (front.y, front.z);
    let right = p.get("right_front_leg");
    (right.y, right.z) = (y, z);
    let stand = g.stand_angle * standing;
    let bob = cos(age * 0.6 + PI);
    p.x_rot("left_hind_leg", stand - leg_anim * 0.5 * speed * i_standing);
    p.x_rot("right_hind_leg", stand + leg_anim * 0.5 * speed * i_standing);
    p.x_rot("left_front_leg", (g.stand_x_rot + bob) * standing + leg_x * i_standing);
    p.x_rot("right_front_leg", (g.stand_x_rot - bob) * standing - leg_x * i_standing);
    if donkey && baby {
        let left = p.get("left_hind_leg");
        left.y = lerp(standing, left.y, -0.3);
        let y = left.y;
        let right = p.get("right_hind_leg");
        right.y = lerp(standing, y, -0.3);
    }
    let scale = s.age_scale();
    let tail = p.get("tail");
    tail.x_rot = g.tail_x_rot + pi_div(6.0) + speed * 0.75;
    tail.y += speed * scale;
    tail.z += speed * 2.0 * scale;
    tail.y_rot = if s.flag("animateTail") { cos(age * 0.7) } else { 0.0 };
    if donkey {
        let chest = s.flag("hasChest");
        p.set_visible("left_chest", chest);
        p.set_visible("right_chest", chest);
    }
    if donkey && baby {
        // BabyDonkeyModel overwrites the state's pitch with -30 and recomputes the head.
        let head_x = -30.0 * pi_div(180.0);
        let base = base_head_angle(s, head_x);
        p.x_rot("head_parts", standing * (pi_div(12.0) + head_x) + eating * (PI / 2.0 + sin(age) * 0.05) + base);
    }
}
