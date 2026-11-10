//! `BlockGetter.forEachBlockIntersectedBetween`: the cells a box sweeps along a move, in vanilla's order.

use std::collections::HashSet;

use crate::aabb::Aabb;
use crate::mth::{floor, length_sqr};

/// Calls `visit(cell, iteration)` for each cell the box ending at `target` passes through on the way from `from` to
/// `to`; `visit` returning false stops the walk.
pub(crate) fn for_each_cell_between(from: [f64; 3], to: [f64; 3], target: &Aabb, mut visit: impl FnMut([i32; 3], u32) -> bool) {
    let travel = sub(to, from);
    if length_sqr(travel) < (1.0E-5_f32 * 1.0E-5_f32) as f64 {
        let (low, high) = (target.min.map(floor), target.max.map(floor));
        for z in low[2]..=high[2] {
            for y in low[1]..=high[1] {
                for x in low[0]..=high[0] {
                    if !visit([x, y, z], 0) {
                        return;
                    }
                }
            }
        }
        return;
    }
    let mut seen = HashSet::new();
    let start = target.moved(travel.map(|v| -v));
    for cell in corners_in_direction(start.min.map(floor), start.max.map(floor), travel) {
        if !visit(cell, 0) {
            return;
        }
        seen.insert(cell);
    }
    let Some(iterations) = along_travel(&mut seen, travel, target, &mut visit) else { return };
    for cell in corners_in_direction(target.min.map(floor), target.max.map(floor), travel) {
        if seen.insert(cell) && !visit(cell, iterations + 1) {
            return;
        }
    }
}

/// `addCollisionsAlongTravel`: steps the leading corner cell by cell, visiting the box's cross-section at each;
/// `None` when `visit` stopped the walk.
fn along_travel(seen: &mut HashSet<[i32; 3]>, travel: [f64; 3], target: &Aabb, visit: &mut impl FnMut([i32; 3], u32) -> bool) -> Option<u32> {
    let size = sub(target.max, target.min);
    let corner = furthest_corner(travel);
    let center = [0, 1, 2].map(|a| (target.min[a] + target.max[a]) * 0.5);
    let to_corner = [0, 1, 2].map(|a| center[a] + size[a] * 0.5 * corner[a] as f64);
    let from_corner = sub(to_corner, travel);
    let mut cell = from_corner.map(floor);
    let sign = travel.map(|v| if v > 0.0 { 1 } else if v < 0.0 { -1 } else { 0 });
    let t_delta = [0, 1, 2].map(|a| if sign[a] == 0 { f64::MAX } else { sign[a] as f64 / travel[a] });
    let frac = |v: f64| v - v.floor();
    let mut t = [0, 1, 2].map(|a| t_delta[a] * if sign[a] > 0 { 1.0 - frac(from_corner[a]) } else { frac(from_corner[a]) });
    let mut iterations = 0;
    while t[0] <= 1.0 || t[1] <= 1.0 || t[2] <= 1.0 {
        let axis = if t[0] < t[1] {
            if t[0] < t[2] { 0 } else { 2 }
        } else if t[1] < t[2] {
            1
        } else {
            2
        };
        cell[axis] += sign[axis];
        t[axis] += t_delta[axis];
        let block = Aabb { min: cell.map(f64::from), max: cell.map(|v| f64::from(v) + 1.0) };
        let Some(hit) = clip(&block, from_corner, to_corner) else { continue };
        iterations += 1;
        let margin = 1.0E-5_f32 as f64;
        let hit = [0, 1, 2].map(|a| hit[a].clamp(cell[a] as f64 + margin, cell[a] as f64 + 1.0 - margin));
        let opposite = [0, 1, 2].map(|a| floor(hit[a] - size[a] * corner[a] as f64));
        for c in corners_in_direction(cell, opposite, travel) {
            if seen.insert(c) && !visit(c, iterations) {
                return None;
            }
        }
    }
    Some(iterations)
}

/// `getFurthestCorner`.
fn furthest_corner(d: [f64; 3]) -> [i32; 3] {
    let [x, y, z] = d.map(f64::abs);
    let s = d.map(|v| if v >= 0.0 { 1 } else { -1 });
    if x <= y && x <= z {
        [-s[0], -s[2], s[1]]
    } else if y <= z {
        [s[2], -s[1], -s[0]]
    } else {
        [-s[1], s[0], -s[2]]
    }
}

/// `BlockPos.betweenCornersInDirection`: the box of cells, walked from the corner `direction` leaves, the axes in
/// `Direction.axisStepOrder` with the last one fastest.
fn corners_in_direction(a: [i32; 3], b: [i32; 3], direction: [f64; 3]) -> Vec<[i32; 3]> {
    let (low, high) = ([0, 1, 2].map(|i| a[i].min(b[i])), [0, 1, 2].map(|i| a[i].max(b[i])));
    let start = [0, 1, 2].map(|i| if direction[i] >= 0.0 { low[i] } else { high[i] });
    let step = direction.map(|v| if v >= 0.0 { 1 } else { -1 });
    let order = if direction[0].abs() < direction[2].abs() { [1, 2, 0] } else { [1, 0, 2] };
    let mut cells = Vec::new();
    for i in 0..=high[order[0]] - low[order[0]] {
        for j in 0..=high[order[1]] - low[order[1]] {
            for k in 0..=high[order[2]] - low[order[2]] {
                let mut cell = start;
                for (axis, n) in [(order[0], i), (order[1], j), (order[2], k)] {
                    cell[axis] += step[axis] * n;
                }
                cells.push(cell);
            }
        }
    }
    cells
}

/// `AABB.clip`: where the segment first enters `area`.
fn clip(area: &Aabb, from: [f64; 3], to: [f64; 3]) -> Option<[f64; 3]> {
    let d = sub(to, from);
    let mut scale = 1.0;
    let mut hit = false;
    // Each axis as (a, b, c): the face's axis, then the other two in vanilla's argument order.
    for (a, b, c) in [(0, 1, 2), (1, 2, 0), (2, 0, 1)] {
        let face = if d[a] > 1.0E-7 {
            area.min[a]
        } else if d[a] < -1.0E-7 {
            area.max[a]
        } else {
            continue;
        };
        let s = (face - from[a]) / d[a];
        let pb = from[b] + s * d[b];
        let pc = from[c] + s * d[c];
        let inside = |p: f64, axis: usize| area.min[axis] - 1.0E-7 < p && p < area.max[axis] + 1.0E-7;
        if 0.0 < s && s < scale && inside(pb, b) && inside(pc, c) {
            scale = s;
            hit = true;
        }
    }
    hit.then(|| [0, 1, 2].map(|i| from[i] + scale * d[i]))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_fall_visits_every_cell_on_the_way() {
        let target = Aabb { min: [0.2, 0.2, 0.2], max: [0.8, 1.0, 0.8] };
        let mut ys = Vec::new();
        for_each_cell_between([0.5, 5.2, 0.5], [0.5, 0.2, 0.5], &target, |cell, _| {
            ys.push(cell[1]);
            true
        });
        ys.sort_unstable();
        ys.dedup();
        assert_eq!(ys, [0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn standing_still_walks_x_fastest() {
        let target = Aabb { min: [0.5, 0.5, 0.5], max: [1.5, 0.9, 1.5] };
        let mut cells = Vec::new();
        for_each_cell_between([1.0; 3], [1.0; 3], &target, |cell, _| {
            cells.push(cell);
            true
        });
        assert_eq!(cells, [[0, 0, 0], [1, 0, 0], [0, 0, 1], [1, 0, 1]]);
    }
}
