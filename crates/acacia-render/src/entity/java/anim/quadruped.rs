//! Four- and eight-legged families: `QuadrupedModel`, `SheepModel`, `ChickenModel`,
//! `CreeperModel`, `SpiderModel`. Ported from Horizon's `mobs/anim.rs` (MIT).

use std::f32::consts::PI;

use super::super::mth::{cos, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

/// `QuadrupedModel.setupAnim`.
pub(super) fn quadruped(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let (pos, sp) = (s.walk_pos(), s.walk_speed());
    p.x_rot("right_hind_leg", cos(pos * 0.6662) * 1.4 * sp);
    p.x_rot("left_hind_leg", cos(pos * 0.6662 + PI) * 1.4 * sp);
    p.x_rot("right_front_leg", cos(pos * 0.6662 + PI) * 1.4 * sp);
    p.x_rot("left_front_leg", cos(pos * 0.6662) * 1.4 * sp);
}

/// `SheepModel.setupAnim`: the head lowered while eating grass.
pub(super) fn sheep(p: &mut Poses, s: &RenderState) {
    quadruped(p, s);
    p.get("head").y += s.num("headEatPositionScale") * 9.0 * s.age_scale();
    p.x_rot("head", s.num("headEatAngleScale"));
}

/// `Sheep.getHeadEatPositionScale` and `getHeadEatAngleScale`, which the renderer copies into
/// the render state: from the ticks left of eating grass (40 at the start), the frame's partial
/// tick and the head's pitch in degrees.
pub fn sheep_eat_scales(eat_tick: i32, partial: f32, x_rot: f32) -> (f32, f32) {
    let t = eat_tick as f32;
    let position = match eat_tick {
        ..=0 => 0.0,
        4..=36 => 1.0,
        1..4 => (t - partial) / 4.0,
        _ => -(t - 40.0 - partial) / 4.0,
    };
    let angle = if (5..=36).contains(&eat_tick) {
        PI / 5.0 + 0.219_911_49 * sin((t - 4.0 - partial) / 32.0 * 28.7)
    } else if eat_tick > 0 {
        PI / 5.0
    } else {
        x_rot.to_radians()
    };
    (position, angle)
}

/// `ChickenModel.setupAnim`.
pub(super) fn chicken(p: &mut Poses, s: &RenderState) {
    let angle = (sin(s.num("flap")) + 1.0) * s.num("flapSpeed");
    p.x_rot("right_leg", cos(s.walk_pos() * 0.6662) * 1.4 * s.walk_speed());
    p.x_rot("left_leg", cos(s.walk_pos() * 0.6662 + PI) * 1.4 * s.walk_speed());
    p.z_rot("right_wing", angle);
    p.z_rot("left_wing", -angle);
    if !s.baby() {
        head(p, s);
    }
}

/// `CreeperModel.setupAnim`. Its fields bind to the opposite side's parts, so the left formulas
/// drive the right legs and the reverse.
pub(super) fn creeper(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let (pos, sp) = (s.walk_pos(), s.walk_speed());
    p.x_rot("left_hind_leg", cos(pos * 0.6662) * 1.4 * sp);
    p.x_rot("right_hind_leg", cos(pos * 0.6662 + PI) * 1.4 * sp);
    p.x_rot("left_front_leg", cos(pos * 0.6662 + PI) * 1.4 * sp);
    p.x_rot("right_front_leg", cos(pos * 0.6662) * 1.4 * sp);
}

/// `SpiderModel.setupAnim`: each pair of legs swings about y and lifts about z, out of phase.
pub(super) fn spider(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let (pos, sp) = (s.walk_pos() * 0.6662, s.walk_speed());
    let swing = |o: f32| -(cos(pos * 2.0 + o) * 0.4) * sp;
    let step = |o: f32| (sin(pos + o) * 0.4).abs() * sp;
    for (legs, offset) in [("hind", 0.0), ("middle_hind", PI), ("middle_front", PI / 2.0), ("front", PI * 3.0 / 2.0)] {
        let (sw, st) = (swing(offset), step(offset));
        let right = p.get(&format!("right_{legs}_leg"));
        right.y_rot += sw;
        right.z_rot += st;
        let left = p.get(&format!("left_{legs}_leg"));
        left.y_rot -= sw;
        left.z_rot -= st;
    }
}
