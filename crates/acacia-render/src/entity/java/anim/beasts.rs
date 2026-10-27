//! `QuadrupedModel` subclasses with poses of their own (polar bear, goat, turtle, panda) and
//! `HoglinModel` (26.3 client).

use std::f32::consts::PI;

use super::super::mth::{cos, lerp, pi_div, rot_lerp_rad, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::quadruped::quadruped;

const LEGS: [&str; 4] = ["right_hind_leg", "left_hind_leg", "right_front_leg", "left_front_leg"];

/// `PolarBearModel`: rears up on its hind legs.
pub(super) fn polar_bear(p: &mut Poses, s: &RenderState) {
    quadruped(p, s);
    let stand = s.num("standScale") * s.num("standScale");
    let age = s.age_scale();
    p.get("body").x_rot -= stand * PI * 0.35;
    p.get("body").y += stand * age * 2.0;
    let right = p.get("right_front_leg");
    right.y -= stand * age * 20.0;
    right.z += stand * age * 4.0;
    right.x_rot -= stand * PI * 0.45;
    let (y, z) = (right.y, right.z);
    let left = p.get("left_front_leg");
    (left.y, left.z) = (y, z);
    left.x_rot -= stand * PI * 0.45;
    let head = p.get("head");
    head.y -= stand * 24.0;
    head.z += stand * 13.0;
    head.x_rot += stand * PI * 0.15;
}

/// `GoatModel` and `BabyGoatModel`: horns shown as the goat has them, the head lowered to ram.
pub(super) fn goat(p: &mut Poses, s: &RenderState) {
    quadruped(p, s);
    let horn = |name: &str| !s.has(name) || s.flag(name);
    p.set_visible("left_horn", horn("hasLeftHorn"));
    p.set_visible("right_horn", horn("hasRightHorn"));
    let ram = s.num("rammingXHeadRot");
    if ram != 0.0 {
        p.x_rot("head", ram);
    } else if s.baby() {
        p.x_rot("head", pi_div(8.0));
    }
}

/// `TurtleModel` and `AdultTurtleModel`: flippers paddle on land, sweep in water.
pub(super) fn turtle(p: &mut Poses, s: &RenderState) {
    quadruped(p, s);
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    if s.flag("isOnLand") {
        let (lay, amplitude) = if s.flag("isLayingEgg") { (4.0, 2.0) } else { (1.0, 1.0) };
        let swing = pos * 5.0;
        let (front, hind) = (cos(lay * swing), cos(swing));
        p.y_rot("right_front_leg", -front * 8.0 * speed * amplitude);
        p.y_rot("left_front_leg", front * 8.0 * speed * amplitude);
        p.y_rot("right_hind_leg", -hind * 3.0 * speed);
        p.y_rot("left_hind_leg", hind * 3.0 * speed);
    } else {
        let swing = cos(pos * 0.6662 * 0.6) * (0.5 * speed);
        p.x_rot("right_hind_leg", swing);
        p.x_rot("left_hind_leg", -swing);
        p.z_rot("right_front_leg", -swing);
        p.z_rot("left_front_leg", swing);
    }
    if !s.baby() {
        let egg = s.flag("hasEgg");
        p.set_visible("egg_belly", egg);
        if egg {
            p.get("root").y -= 1.0;
        }
    }
}

/// `PandaModel` and `BabyPandaModel`: unhappy, sneezing, sitting to eat, rolling, lying back.
pub(super) fn panda(p: &mut Poses, s: &RenderState) {
    quadruped(p, s);
    let age = s.age();
    if s.flag("isUnhappy") {
        p.y_rot("head", 0.35 * sin(0.6 * age));
        p.z_rot("head", 0.35 * sin(0.6 * age));
        p.x_rot("right_front_leg", -0.75 * sin(0.3 * age));
        p.x_rot("left_front_leg", 0.75 * sin(0.3 * age));
    } else {
        p.z_rot("head", 0.0);
    }
    if s.flag("isSneezing") {
        let time = s.num("sneezeTime") as i32;
        if time < 15 {
            p.x_rot("head", pi_div(-4.0) * time as f32 / 14.0);
        } else if time < 20 {
            // Integer division in Java: always 0 here.
            let along = ((time - 15) / 5) as f32;
            p.x_rot("head", pi_div(-4.0) + pi_div(4.0) * along);
        }
    }
    let sit = s.num("sitAmount");
    if sit > 0.0 {
        panda_sitting(p, sit, s.baby());
        let chew = sin(age * 0.6);
        if s.flag("isEating") {
            p.x_rot("head", PI / 2.0 + 0.2 * chew);
            p.x_rot("right_front_leg", -0.4 - 0.2 * chew);
            p.x_rot("left_front_leg", -0.4 - 0.2 * chew);
        }
        if s.flag("isScared") {
            p.x_rot("head", 2.170_796_4);
            p.x_rot("right_front_leg", -0.9);
            p.x_rot("left_front_leg", -0.9);
        }
    } else {
        for leg in LEGS {
            p.z_rot(leg, 0.0);
        }
    }
    let lie = s.num("lieOnBackAmount");
    if lie > 0.0 {
        p.x_rot("right_hind_leg", -0.6 * sin(age * 0.15));
        p.x_rot("left_hind_leg", 0.6 * sin(age * 0.15));
        p.x_rot("right_front_leg", 0.3 * sin(age * 0.25));
        p.x_rot("left_front_leg", -0.3 * sin(age * 0.25));
        let x = p.read("head").x_rot;
        p.x_rot("head", rot_lerp_rad(lie, x, PI / 2.0));
    }
    let roll = s.num("rollAmount");
    if roll > 0.0 {
        let x = p.read("head").x_rot;
        p.x_rot("head", rot_lerp_rad(roll, x, 2.056_194_5));
        p.x_rot("right_hind_leg", -0.5 * sin(age * 0.5));
        p.x_rot("left_hind_leg", 0.5 * sin(age * 0.5));
        p.x_rot("right_front_leg", 0.5 * sin(age * 0.5));
        p.x_rot("left_front_leg", -0.5 * sin(age * 0.5));
    }
}

/// `animateSitting`; the baby's also slides its parts into a sitting shape.
fn panda_sitting(p: &mut Poses, sit: f32, baby: bool) {
    let body = p.get("body");
    body.x_rot = rot_lerp_rad(sit, body.x_rot, if baby { pi_div(18.0) } else { 1.740_796_3 });
    if baby {
        body.z = lerp(sit, body.z, -1.5);
        let head = p.get("head");
        head.z = lerp(sit, head.z, -11.5);
        head.y = lerp(sit, head.y, 17.5);
        for (leg, z) in [("right_front_leg", -5.0), ("left_front_leg", -5.0), ("right_hind_leg", 3.0), ("left_hind_leg", 3.0)] {
            let part = p.get(leg);
            part.z = lerp(sit, part.z, z);
        }
    } else {
        let head = p.get("head");
        head.x_rot = rot_lerp_rad(sit, head.x_rot, PI / 2.0);
    }
    for (leg, z) in [("right_front_leg", -0.270_796_42), ("left_front_leg", 0.270_796_42), ("right_hind_leg", 0.570_796_4), ("left_hind_leg", -0.570_796_4)] {
        p.z_rot(leg, z);
    }
}

/// `HoglinModel` (zoglins too): ears flick with the walk, the head swings up in a headbutt; a
/// baby's head also rises.
pub(super) fn hoglin(p: &mut Poses, s: &RenderState) {
    let (speed, pos) = (s.walk_speed(), s.walk_pos());
    p.z_rot("right_ear", (-std::f64::consts::PI * 2.0 / 9.0) as f32 - speed * sin(pos));
    p.z_rot("left_ear", (std::f64::consts::PI * 2.0 / 9.0) as f32 + speed * sin(pos));
    p.y_rot("head", s.y_rot() * pi_div(180.0));
    let butt = 1.0 - (10 - 2 * s.num("attackAnimationRemainingTicks") as i32).abs() as f32 / 10.0;
    p.x_rot("head", lerp(butt, 0.872_664_63, pi_div(-9.0)));
    if s.baby() {
        p.get("head").y += butt * 2.5;
    }
    p.x_rot("right_front_leg", cos(pos) * 1.2 * speed);
    p.x_rot("left_front_leg", cos(pos + PI) * 1.2 * speed);
    let (right, left) = (p.read("right_front_leg").x_rot, p.read("left_front_leg").x_rot);
    p.x_rot("right_hind_leg", left);
    p.x_rot("left_hind_leg", right);
}
