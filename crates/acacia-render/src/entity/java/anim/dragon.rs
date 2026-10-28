//! `EnderDragonModel` (26.3 client): the neck and tail follow the flight history, the wings
//! and legs the flap.

use super::super::mth::{cos, pi_div, rot_lerp, sin, wrap_degrees};
use super::super::poses::Poses;
use super::super::state::RenderState;

/// `DragonFlightHistory` filled by `record` from the state's `flightHistory` (y, yRot pairs, oldest
/// first).
struct History {
    samples: [(f64, f32); 64],
    head: i32,
    partial: f32,
}

impl History {
    fn of(s: &RenderState) -> History {
        let mut out = History { samples: [(0.0, 0.0); 64], head: -1, partial: s.num("partialTicks") };
        for pair in s.nums("flightHistory").as_chunks::<2>().0 {
            let sample = (pair[0], pair[1] as f32);
            if out.head < 0 {
                out.samples = [sample; 64];
            }
            out.head = (out.head + 1) % 64;
            out.samples[out.head as usize] = sample;
        }
        out
    }

    fn at(&self, delay: i32) -> (f64, f32) {
        self.samples[((self.head - delay) & 63) as usize]
    }

    /// `EnderDragonRenderState.getHistoricalPos`.
    fn get(&self, delay: i32) -> (f64, f32) {
        let (now, old) = (self.at(delay), self.at(delay + 1));
        (old.0 + f64::from(self.partial) * (now.0 - old.0), rot_lerp(self.partial, old.1, now.1))
    }
}

/// `EnderDragonRenderState.getHeadPartYOffset`.
fn head_part_y_offset(s: &RenderState, part: i32, body: (f64, f32), at: (f64, f32)) -> f32 {
    let result = if s.flag("isLandingOrTakingOff") {
        f64::from(part) / (f64::from(s.num("distanceToEgg")) / 4.0).max(1.0)
    } else if s.flag("isSitting") {
        f64::from(part)
    } else if part == 6 {
        0.0
    } else {
        at.0 - body.0
    };
    result as f32
}

pub(super) fn ender_dragon(p: &mut Poses, s: &RenderState) {
    let deg = pi_div(180.0);
    let flap = s.num("flapTime") * pi_div(0.5);
    p.x_rot("jaw", (sin(flap) + 1.0) * 0.2);
    let mut bounce = sin(flap - 1.0) + 1.0;
    bounce = (bounce * bounce + bounce * 2.0) * 0.05;
    let root = p.get("root");
    (root.y, root.z, root.x_rot) = ((bounce - 2.0) * 16.0, -48.0, bounce * 2.0 * deg);
    let neck0 = p.read("neck0");
    let (mut xx, mut yy, mut zz) = (neck0.x, neck0.y, neck0.z);
    let h = History::of(s);
    let mut start = h.get(6);
    let rot2 = wrap_degrees(h.get(5).1 - h.get(10).1);
    let rot = wrap_degrees(h.get(5).1 + rot2 / 2.0);
    for i in 0..5 {
        let point = h.get(5 - i);
        let neck_x_rot = cos(i as f32 * 0.45 + flap) * 0.15;
        let neck = p.get(&format!("neck{i}"));
        neck.y_rot = wrap_degrees(point.1 - start.1) * deg * 1.5;
        neck.x_rot = neck_x_rot + head_part_y_offset(s, i, start, point) * deg * 1.5 * 5.0;
        neck.z_rot = -wrap_degrees(point.1 - rot) * deg * 1.5;
        (neck.x, neck.y, neck.z) = (xx, yy, zz);
        let (x_rot, y_rot) = (neck.x_rot, neck.y_rot);
        xx -= sin(y_rot) * cos(x_rot) * 10.0;
        yy += sin(x_rot) * 10.0;
        zz -= cos(y_rot) * cos(x_rot) * 10.0;
    }
    let current = h.get(0);
    let head = p.get("head");
    (head.x, head.y, head.z) = (xx, yy, zz);
    head.y_rot = wrap_degrees(current.1 - start.1) * deg;
    head.x_rot = wrap_degrees(head_part_y_offset(s, 6, start, current)) * deg * 1.5 * 5.0;
    head.z_rot = -wrap_degrees(current.1 - rot) * deg;
    p.z_rot("body", -rot2 * 1.5 * deg);
    let wing = (0.125 - cos(flap) * 0.2, -0.25, -(sin(flap) + 0.125) * 0.8);
    let tip = (sin(flap + 2.0) + 0.5) * 0.75;
    let left = p.get("left_wing");
    (left.x_rot, left.y_rot, left.z_rot) = wing;
    p.z_rot("left_wing_tip", tip);
    let right = p.get("right_wing");
    (right.x_rot, right.y_rot, right.z_rot) = (wing.0, -wing.1, -wing.2);
    p.z_rot("right_wing_tip", -tip);
    for side in ["left", "right"] {
        let lift = bounce * 0.1;
        p.x_rot(&format!("{side}_hind_leg"), 1.0 + lift);
        p.x_rot(&format!("{side}_hind_leg_tip"), 0.5 + lift);
        p.x_rot(&format!("{side}_hind_foot"), 0.75 + lift);
        p.x_rot(&format!("{side}_front_leg"), 1.3 + lift);
        p.x_rot(&format!("{side}_front_leg_tip"), -0.5 - lift);
        p.x_rot(&format!("{side}_front_foot"), 0.75 + lift);
    }
    let mut tail_x_rot = 0.0;
    let tail0 = p.read("tail0");
    (xx, yy, zz) = (tail0.x, tail0.y, tail0.z);
    start = h.get(11);
    for i in 0..12 {
        let point = h.get(12 + i);
        tail_x_rot += sin(i as f32 * 0.45 + flap) * 0.05;
        let tail = p.get(&format!("tail{i}"));
        tail.y_rot = (wrap_degrees(point.1 - start.1) * 1.5 + 180.0) * deg;
        tail.x_rot = tail_x_rot + (point.0 - start.0) as f32 * deg * 1.5 * 5.0;
        tail.z_rot = wrap_degrees(point.1 - rot) * deg * 1.5;
        (tail.x, tail.y, tail.z) = (xx, yy, zz);
        let (x_rot, y_rot) = (tail.x_rot, tail.y_rot);
        yy += sin(x_rot) * 10.0;
        zz -= cos(y_rot) * cos(x_rot) * 10.0;
        xx -= sin(y_rot) * cos(x_rot) * 10.0;
    }
}
