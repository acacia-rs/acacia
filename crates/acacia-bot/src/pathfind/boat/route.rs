//! A boat route over still water: A* over block corners (a centred boat box spans the 2×2 columns around one),
//! then any-angle smoothing by clearance-checked line of sight. One layer and no rollouts, so it is cheap.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use acacia_physics::WorldView;

use super::grid::WaterGrid;

/// Corners expanded before the search gives up.
const MAX_EXPANSIONS: usize = 400_000;
/// Extra cost per block for a corner whose padded box touches a bank: the route keeps to open water.
const NEAR_BANK: f32 = 0.6;
/// The padding that counts as near a bank, past the route's own.
const BANK_ROOM: f32 = 0.5;
/// How far a corner may be from the boat to start the route from.
const START_REACH: i32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Water block level the route floats on.
    pub y: i32,
    /// Boat centres, start first; consecutive points see each other.
    pub points: Vec<[f32; 2]>,
    /// Ends at the stop; a partial route ends where the known water stops leading towards it.
    pub complete: bool,
}

impl Route {
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum()
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Open(f32, (i32, i32));

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0)
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Corners this far from the stop may end the corner path, if they see it.
const STOP_REACH: f32 = 2.0;

/// Clearance a smoothed shortcut keeps past the route's padding when it can.
const SMOOTH_ROOM: f32 = 0.6;
/// Blocks closer to the stop a partial route must get.
const MIN_GAIN: f32 = 8.0;

/// A route for a boat at `from` to the stop `to` (boat centres, horizontal), keeping `pad` from the banks; the stop
/// itself only needs the boat's own box clear. When the known water does not reach it, a partial route to the
/// corner nearest it ([`Route::complete`] false), or `None` when that gains under `MIN_GAIN`.
pub fn plan<W: WorldView + ?Sized>(grid: &WaterGrid<'_, W>, from: [f32; 2], to: [f32; 2], pad: f32) -> Option<Route> {
    let start = start_corner(grid, from, pad)?;
    let corner = |(x, z): (i32, i32)| [x as f32, z as f32];
    let dist = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let h = |c: (i32, i32)| (octile(corner(c), to) - STOP_REACH).max(0.0);
    let mut open = BinaryHeap::from([Open(h(start), start)]);
    let mut best: HashMap<(i32, i32), (f32, (i32, i32))> = HashMap::from([(start, (0.0, start))]);
    let (mut expanded, mut nearest) = (0, (h(start), start));
    while let Some(Open(f, c)) = open.pop() {
        let g = best[&c].0;
        if f > g + h(c) {
            continue;
        }
        if dist(corner(c), to) <= STOP_REACH && grid.line_clear(corner(c), to, 0.0) {
            let mut corners: Vec<[f32; 2]> = walk_back(&best, c).into_iter().map(corner).collect();
            corners.push(to);
            return Some(smooth(grid, from, corners, pad, true));
        }
        if h(c) < nearest.0 {
            nearest = (h(c), c);
        }
        expanded += 1;
        if expanded > MAX_EXPANSIONS {
            break;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let n = (c.0 + dx, c.1 + dz);
            // A diagonal needs both sides open, or its line would clip the corner between them.
            let sides = dx == 0 || dz == 0 || (passable(grid, (c.0 + dx, c.1), pad) && passable(grid, (c.0, c.1 + dz), pad));
            if !sides || !passable(grid, n, pad) {
                continue;
            }
            let step = if dx != 0 && dz != 0 { std::f32::consts::SQRT_2 } else { 1.0 };
            let bank = if grid.clear(n.0 as f32, n.1 as f32, pad + BANK_ROOM) { 0.0 } else { NEAR_BANK };
            let ng = g + step * (1.0 + bank);
            if best.get(&n).is_none_or(|&(old, _)| ng < old) {
                best.insert(n, (ng, c));
                open.push(Open(ng + h(n), n));
            }
        }
    }
    // Short of the stop (unknown water ahead, or none): as far towards it as the known water goes.
    (h(start) - nearest.0 >= MIN_GAIN).then(|| smooth(grid, from, walk_back(&best, nearest.1).into_iter().map(corner).collect(), pad, false))
}

fn passable<W: WorldView + ?Sized>(grid: &WaterGrid<'_, W>, (x, z): (i32, i32), pad: f32) -> bool {
    grid.clear(x as f32, z as f32, pad)
}

fn octile(a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dz) = ((a[0] - b[0]).abs(), (a[1] - b[1]).abs());
    dx.max(dz) + (std::f32::consts::SQRT_2 - 1.0) * dx.min(dz)
}

/// The nearest passable corner the boat sees from where it floats.
fn start_corner<W: WorldView + ?Sized>(grid: &WaterGrid<'_, W>, from: [f32; 2], pad: f32) -> Option<(i32, i32)> {
    let (cx, cz) = (from[0].round() as i32, from[1].round() as i32);
    let mut found: Option<(f32, (i32, i32))> = None;
    for x in cx - START_REACH..=cx + START_REACH {
        for z in cz - START_REACH..=cz + START_REACH {
            let d = (x as f32 - from[0]).hypot(z as f32 - from[1]);
            if found.is_some_and(|(b, _)| b <= d) || !passable(grid, (x, z), pad) {
                continue;
            }
            if grid.line_clear(from, [x as f32, z as f32], 0.0) {
                found = Some((d, (x, z)));
            }
        }
    }
    found.map(|(_, c)| c)
}

fn walk_back(best: &HashMap<(i32, i32), (f32, (i32, i32))>, mut c: (i32, i32)) -> Vec<(i32, i32)> {
    let mut out = vec![c];
    while best[&c].1 != c {
        c = best[&c].1;
        out.push(c);
    }
    out.reverse();
    out
}

/// String-pulls the corner path from the boat: each point is the farthest one ahead still in clear sight.
fn smooth<W: WorldView + ?Sized>(grid: &WaterGrid<'_, W>, from: [f32; 2], corners: Vec<[f32; 2]>, pad: f32, complete: bool) -> Route {
    const LOOK: usize = 96;
    let mut all = Vec::with_capacity(corners.len() + 1);
    all.push(from);
    all.extend(corners);
    let mut points = vec![from];
    let mut i = 0;
    while i + 1 < all.len() {
        let last = (i + LOOK).min(all.len() - 1);
        // Shortcuts keep `SMOOTH_ROOM` more where the water allows: a boat cannot turn on a vertex that grazes a bank.
        let seen = |room: f32| (i + 2..=last).rev().find(|&k| grid.line_clear(all[i], all[k], pad + room));
        let j = seen(SMOOTH_ROOM).or_else(|| seen(0.0)).unwrap_or(i + 1);
        points.push(all[j]);
        i = j;
    }
    Route { y: grid.y, points, complete }
}
