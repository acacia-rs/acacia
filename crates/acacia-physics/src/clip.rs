//! Port of bedsim `collision.go` (vanilla `AABB::clipCollide`) and the auto-step sweep.

use crate::aabb::Aabb;
use crate::math::{Vec3, add};

/// Contact distances within one micrometre snap to zero.
const CONTACT_EPSILON: f32 = 1e-6;

struct ClipResult {
    depenetrating_axis: usize,
    penetration: f32,
    clipped: Vec3,
    depenetrating: Vec3,
}

/// Clips (or, unless `one_way`, depenetrates) `vel` of `moving` against `stationary`,
/// recording the deepest penetration per axis into `penetration`.
pub fn clip_collide(stationary: &Aabb, moving: &Aabb, vel: Vec3, one_way: bool, penetration: Option<&mut Vec3>) -> Vec3 {
    let r = do_clip(stationary, moving, vel);
    if let Some(p) = penetration
        && p[r.depenetrating_axis] < r.penetration
    {
        p[r.depenetrating_axis] = r.penetration;
    }
    if one_way { r.clipped } else { r.depenetrating }
}

fn do_clip(stationary: &Aabb, moving: &Aabb, velocity: Vec3) -> ClipResult {
    let mut r = ClipResult { depenetrating_axis: 0, penetration: 0.0, clipped: velocity, depenetrating: velocity };
    if stationary.is_empty() {
        return r;
    }
    let mut pens = [0f32; 3];
    let mut signed = [0f32; 3];
    let mut normals = [0f32; 3];
    let (mut separating, mut sep_axis) = (0, 0);
    let mut result_pen = f32::MAX;
    for i in 0..3 {
        let mut min_pen = moving.max[i] - stationary.min[i];
        let mut max_pen = stationary.max[i] - moving.min[i];
        if min_pen.abs() <= CONTACT_EPSILON {
            min_pen = 0.0;
        }
        if max_pen.abs() <= CONTACT_EPSILON {
            max_pen = 0.0;
        }
        let min_pos = min_pen.max(0.0);
        let max_pos = max_pen.max(0.0);
        if min_pos == 0.0 {
            pens[i] = 0.0;
            signed[i] = min_pen;
            normals[i] = -1.0;
            separating += 1;
            sep_axis = i;
        } else if max_pos == 0.0 {
            pens[i] = 0.0;
            signed[i] = max_pen;
            normals[i] = 1.0;
            separating += 1;
            sep_axis = i;
        } else if min_pos < max_pos {
            pens[i] = min_pos;
            signed[i] = min_pos;
            normals[i] = -1.0;
        } else {
            pens[i] = max_pos;
            signed[i] = max_pos;
            normals[i] = 1.0;
        }
        if separating > 1 {
            return r;
        }
        result_pen = result_pen.min(pens[i]);
    }

    if separating == 0 {
        r.penetration = result_pen;
        let mut best = 0;
        for i in 1..3 {
            if pens[i] < pens[best] {
                best = i;
            }
        }
        let desired = pens[best] * normals[best];
        r.depenetrating[best] =
            if desired > 0.0 { desired.max(velocity[best]) } else { desired.min(velocity[best]) };
        r.depenetrating_axis = best;
        // BDS with no depenetration allowed (a player): out along the shallowest axis is free, further in is not.
        r.clipped[best] = if normals[best] > 0.0 { velocity[best].max(0.0) } else { velocity[best].min(0.0) };
        return r;
    }

    let swept = signed[sep_axis] - normals[sep_axis] * velocity[sep_axis];
    if swept <= 0.0 {
        return r;
    }
    let resolved = signed[sep_axis] * normals[sep_axis];
    r.clipped[sep_axis] = resolved;
    r.depenetrating[sep_axis] = resolved;
    r
}

/// Clips `vel` against every box, iterating in reverse like the client.
pub(crate) fn clip_all(boxes: &[Aabb], bb: &Aabb, mut vel: Vec3, one_way: bool, mut pen: Option<&mut Vec3>) -> Vec3 {
    for b in boxes.iter().rev() {
        vel = clip_collide(b, bb, vel, one_way, pen.as_deref_mut());
    }
    vel
}

pub(crate) struct AutoStep {
    pub bb: Aabb,
    pub velocity: Vec3,
}

/// The client auto-step sequence: up by `height`, X, Z, then back down.
pub(crate) fn auto_step(original: &Aabb, velocity: Vec3, boxes: &[Aabb], one_way: bool, height: f32) -> AutoStep {
    let relevant: Vec<Aabb> = boxes.iter().copied().filter(|b| b.min[1] < original.max[1]).collect();
    let mut bb = *original;
    let up = clip_all(&relevant, &bb, [0.0, height, 0.0], one_way, None);
    bb = bb.translate(up);
    let x = clip_all(&relevant, &bb, [velocity[0], 0.0, 0.0], one_way, None);
    bb = bb.translate(x);
    let z = clip_all(&relevant, &bb, [0.0, 0.0, velocity[2]], one_way, None);
    bb = bb.translate(z);
    let down = clip_all(&relevant, &bb, [-up[0], -up[1], -up[2]], one_way, None);
    bb = bb.translate(down);
    AutoStep { bb, velocity: add(add(add(up, x), z), down) }
}
