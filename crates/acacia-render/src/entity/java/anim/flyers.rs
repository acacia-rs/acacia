//! Swimmers and fliers (26.3 client): `CodModel`, `TropicalFish*Model`, `SalmonModel`,
//! `Pufferfish*Model`, `GuardianModel`, `PhantomModel`, `AllayModel`.

use std::f32::consts::PI;

use super::super::mth::{cos, lerp, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

/// Cod and tropical fish: the tail beats, harder out of water.
pub(super) fn fish(p: &mut Poses, s: &RenderState) {
    let amplitude = if s.flag("isInWater") { 1.0 } else { 1.5 };
    let beat = -amplitude * 0.45 * sin(0.6 * s.age());
    p.y_rot("tail_fin", beat);
    p.y_rot("tail", beat);
}

pub(super) fn tadpole(p: &mut Poses, s: &RenderState) {
    let amplitude = if s.flag("isInWater") { 1.0 } else { 1.5 };
    p.y_rot("tail", -amplitude * 0.25 * sin(0.3 * s.age()));
}

pub(super) fn salmon(p: &mut Poses, s: &RenderState) {
    let (amplitude, angle) = if s.flag("isInWater") { (1.0, 1.0) } else { (1.3, 1.7) };
    p.y_rot("body_back", -amplitude * 0.25 * sin(angle * 0.6 * s.age()));
}

/// Every puff size: the fins (`*_blue_fin` puffed, `*_fin` small) flap.
pub(super) fn pufferfish(p: &mut Poses, s: &RenderState) {
    let flap = 0.4 * sin(s.age() * 0.2);
    for prefix in ["right_blue_fin", "right_fin"] {
        p.z_rot(prefix, -0.2 + flap);
    }
    for prefix in ["left_blue_fin", "left_fin"] {
        p.z_rot(prefix, 0.2 - flap);
    }
}

const SPIKE_X: [f32; 12] = [0.0, 0.0, 8.0, -8.0, -8.0, 8.0, 8.0, -8.0, 0.0, 0.0, 8.0, -8.0];
const SPIKE_Y: [f32; 12] = [-8.0, -8.0, -8.0, -8.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0];
const SPIKE_Z: [f32; 12] = [8.0, -8.0, 0.0, 0.0, -8.0, -8.0, 8.0, 8.0, 8.0, -8.0, 0.0, 0.0];

/// Spikes pulse and withdraw, the tail swims.
// TODO: the eye following its target (`lookAtPosition`); the viewer has no target to give it.
pub(super) fn guardian(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let age = s.age();
    let withdrawal = (1.0 - s.num("spikesAnimation")) * 0.55;
    for i in 0..12 {
        let offset = 1.0 + cos(age * 1.5 + i as f32) * 0.01 - withdrawal;
        let spike = p.get(&format!("spike{i}"));
        (spike.x, spike.y, spike.z) = (SPIKE_X[i] * offset, 16.0 + SPIKE_Y[i] * offset, SPIKE_Z[i] * offset);
    }
    p.set_visible("eye", true);
    let swim = sin(s.num("tailAnimation"));
    for (i, share) in [0.05, 0.1, 0.15].into_iter().enumerate() {
        p.y_rot(&format!("tail{i}"), swim * PI * share);
    }
}

pub(super) fn phantom(p: &mut Poses, s: &RenderState) {
    let anim = s.num("flapTime") * 7.448_451 * pi_div(180.0);
    let wing = cos(anim) * 16.0 * pi_div(180.0);
    for (part, z) in [("left_wing_base", wing), ("left_wing_tip", wing), ("right_wing_base", -wing), ("right_wing_tip", -wing)] {
        p.z_rot(part, z);
    }
    let tail = -(5.0 + cos(anim * 2.0) * 5.0) * pi_div(180.0);
    p.x_rot("tail_base", tail);
    p.x_rot("tail_tip", tail);
}

/// Wings flap, the body leans into flight, the arms bob or hold an item; dancing sways and spins.
pub(super) fn allay(p: &mut Poses, s: &RenderState) {
    let (speed, pos, age) = (s.walk_speed(), s.walk_pos(), s.age());
    let flap = cos(age * 20.0 * pi_div(180.0) + pos) * PI * 0.15 + speed;
    let idle_speed = age * 9.0 * pi_div(180.0);
    let flying = (speed / 0.3).min(1.0);
    let idle = 1.0 - flying;
    let holding = s.num("holdingAnimationProgress");
    if s.flag("isDancing") {
        let dance = age * 8.0 * pi_div(180.0) + speed;
        let spin = s.num("spinningProgress");
        if s.flag("isSpinning") {
            p.y_rot("root", (std::f64::consts::PI * 4.0) as f32 * spin);
        }
        p.z_rot("root", cos(dance) * 16.0 * pi_div(180.0) * (1.0 - spin));
        p.y_rot("head", cos(dance) * 30.0 * pi_div(180.0) * (1.0 - spin));
        p.z_rot("head", cos(dance) * 14.0 * pi_div(180.0) * (1.0 - spin));
    } else {
        head(p, s);
    }
    for (wing, side) in [("right_wing", -1.0), ("left_wing", 1.0)] {
        p.x_rot(wing, 0.436_332_32 * (1.0 - flying));
        p.y_rot(wing, side * pi_div(4.0) - side * flap);
    }
    p.x_rot("body", flying * pi_div(4.0));
    let arm_x = holding * lerp(flying, pi_div(-3.0), -1.134_464);
    // Java's double Math.cos here, not the Mth table.
    p.get("root").y += f64::from(idle_speed).cos() as f32 * 0.25 * idle;
    p.x_rot("right_arm", arm_x);
    p.x_rot("left_arm", arm_x);
    let bob = 0.436_332_32 - cos(idle_speed + (std::f64::consts::PI * 3.0 / 2.0) as f32) * PI * 0.075 * (idle * (1.0 - holding));
    p.z_rot("left_arm", -bob);
    p.z_rot("right_arm", bob);
    p.y_rot("right_arm", 0.279_252_68 * holding);
    p.y_rot("left_arm", -0.279_252_68 * holding);
}
