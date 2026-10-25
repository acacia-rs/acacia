//! Families with a short `setupAnim` of their own: silverfish, endermite, blaze, ghast, golems,
//! villagers. Ported from the 26.3 client's models.

use std::f32::consts::PI;

use super::super::mth::{cos, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

/// `SilverfishModel` (0.05, 0.2) and `EndermiteModel` (0.01, 0.1): the segments sway and slide.
fn crawl(p: &mut Poses, s: &RenderState, segments: i32, turn: f32, slide: f32) {
    for i in 0..segments {
        let phase = s.age() * 0.9 + i as f32 * 0.15 * PI;
        let name = format!("segment{i}");
        p.y_rot(&name, cos(phase) * PI * turn * (1 + (i - 2).abs()) as f32);
        p.get(&name).x = sin(phase) * PI * slide * (i - 2).abs() as f32;
    }
}

pub(super) fn silverfish(p: &mut Poses, s: &RenderState) {
    crawl(p, s, 7, 0.05, 0.2);
    for (layer, segment, moves) in [("layer0", "segment2", false), ("layer1", "segment4", true), ("layer2", "segment1", true)] {
        let from = p.read(segment);
        p.y_rot(layer, from.y_rot);
        if moves {
            p.get(layer).x = from.x;
        }
    }
}

pub(super) fn endermite(p: &mut Poses, s: &RenderState) {
    crawl(p, s, 4, 0.01, 0.1);
}

/// `BlazeModel`: three rings of rods circling at their own speeds and bobbing.
pub(super) fn blaze(p: &mut Poses, s: &RenderState) {
    let age = s.age();
    let rings: [(i32, f32, f32, f32); 3] = [(0, age * PI * -0.1, -2.0, 9.0), (4, PI / 4.0 + age * PI * 0.03, 2.0, 7.0), (8, 0.471_238_94 + age * PI * -0.05, 11.0, 5.0)];
    for (first, mut angle, y, radius) in rings {
        for i in first..first + 4 {
            let bob = if first == 8 { cos((i as f32 * 1.5 + age) * 0.5) } else { cos(((i * 2) as f32 + age) * 0.25) };
            let part = p.get(&format!("part{i}"));
            part.y = y + bob;
            part.x = cos(angle) * radius;
            part.z = sin(angle) * radius;
            angle += PI / 2.0;
        }
    }
    head(p, s);
}

/// `GhastModel.animateTentacles`.
pub(super) fn ghast(p: &mut Poses, s: &RenderState) {
    for i in 0..9 {
        p.x_rot(&format!("tentacle{i}"), 0.2 * sin(s.age() * 0.3 + i as f32) + 0.4);
    }
}

/// `HappyGhastModel`: a harnessed body squeezes in.
pub(super) fn happy_ghast(p: &mut Poses, s: &RenderState) {
    if s.text("bodyItem").is_some_and(|item| !item.is_empty() && item != "minecraft:air") {
        let body = p.get("body");
        (body.x_scale, body.y_scale, body.z_scale) = (0.9375, 0.9375, 0.9375);
    }
    ghast(p, s);
}

/// `HappyGhastHarnessModel`: goggles up on the forehead, down over the eyes while ridden.
pub(super) fn happy_ghast_harness(p: &mut Poses, s: &RenderState) {
    let goggles = p.get("goggles");
    (goggles.x_rot, goggles.y) = if s.flag("isRidden") { (0.0, 14.0) } else { (-0.7854, 9.0) };
}

/// `MagmaCubeModel`: the cube's slices spread apart as it squishes.
pub(super) fn magma_cube(p: &mut Poses, s: &RenderState) {
    let squish = s.num("squish").max(0.0);
    for i in 0..8 {
        p.get(&format!("cube{i}")).y = -(4 - i) as f32 * squish * 1.7;
    }
}

/// `Mth.triangleWave`.
pub(super) fn triangle_wave(index: f32, period: f32) -> f32 {
    ((index % period - period * 0.5).abs() - period * 0.25) / (period * 0.25)
}

/// `IronGolemModel`: arms swing with the walk, raised in an attack, one held out with a flower.
pub(super) fn iron_golem(p: &mut Poses, s: &RenderState) {
    let (attack, speed, pos) = (s.num("attackTicksRemaining"), s.walk_speed(), s.walk_pos());
    let flower = s.num("offerFlowerTick") as i32;
    let (right, left) = if attack > 0.0 {
        let raised = -2.0 + 1.5 * triangle_wave(attack, 10.0);
        (raised, raised)
    } else if flower > 0 {
        (-0.8 + 0.025 * triangle_wave(flower as f32, 70.0), 0.0)
    } else {
        ((-0.2 + 1.5 * triangle_wave(pos, 13.0)) * speed, (-0.2 - 1.5 * triangle_wave(pos, 13.0)) * speed)
    };
    p.x_rot("right_arm", right);
    p.x_rot("left_arm", left);
    head(p, s);
    p.x_rot("right_leg", -1.5 * triangle_wave(pos, 13.0) * speed);
    p.x_rot("left_leg", 1.5 * triangle_wave(pos, 13.0) * speed);
}

/// `SnowGolemModel`: the upper body turns a quarter of the look, the arms with it.
pub(super) fn snow_golem(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let turn = s.y_rot() * super::super::mth::pi_div(180.0) * 0.25;
    p.y_rot("upper_body", turn);
    let (sin, cos) = (sin(turn), cos(turn));
    p.y_rot("left_arm", turn);
    p.y_rot("right_arm", turn + PI);
    let left = p.get("left_arm");
    (left.x, left.z) = (cos * 5.0, -sin * 5.0);
    let right = p.get("right_arm");
    (right.x, right.z) = (-cos * 5.0, sin * 5.0);
}

/// `VillagerModel`: an unhappy villager shakes its head down.
pub(super) fn villager(p: &mut Poses, s: &RenderState) {
    head(p, s);
    if s.flag("isUnhappy") {
        p.z_rot("head", 0.3 * sin(0.45 * s.age()));
        p.x_rot("head", 0.4);
    }
    let (pos, speed) = (s.walk_pos(), s.walk_speed());
    p.x_rot("right_leg", cos(pos * 0.6662) * 1.4 * speed * 0.5);
    p.x_rot("left_leg", cos(pos * 0.6662 + PI) * 1.4 * speed * 0.5);
}
