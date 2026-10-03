//! A* over feet block positions with Baritone's best-partial-path fallback (azalea `astar/mod.rs`).

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

use acacia_physics::{BlockPos, Vec3};

use super::goal::Goal;
use super::kit::Kit;
use super::moves::{self, Ctx, Edge, MoveKind};
use super::terrain::{Blocks, EPS, PosMap, Spot, Terrain};
use super::work::Work;
use super::PathNode;

/// Partial-path candidates are the best nodes by `h + g / c` for each coefficient (Baritone).
const COEFFICIENTS: [f32; 7] = [1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 15.0];
const MIN_IMPROVEMENT: f32 = 0.01;

#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub max_nodes: usize,
    pub budget: Duration,
    /// Allow gap jumps.
    pub parkour: bool,
    /// Break blocks in the way (costed by break time with the best tool in `kit`).
    pub allow_dig: bool,
    /// Place scaffold blocks from `kit` to cross gaps and to pillar up.
    pub allow_bridge: bool,
    /// Open wooden doors, fence gates and trapdoors in the way.
    pub open_doors: bool,
    /// Tools and blocks to plan with; [`Bot::goto`](crate::Bot::goto) fills it from the inventory.
    pub kit: Kit,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            max_nodes: 200_000,
            budget: Duration::from_secs(2),
            parkour: true,
            allow_dig: false,
            allow_bridge: false,
            open_doors: true,
            kit: Kit::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Path {
    /// Nodes after the start, in order.
    pub nodes: Vec<PathNode>,
    /// False when the search ran out of budget or options: `nodes` then leads to the node that
    /// looked closest to the goal.
    pub complete: bool,
    /// Estimated ticks.
    pub cost: f32,
    /// Nodes expanded.
    pub visited: usize,
}

#[derive(Clone, Copy)]
struct Rec {
    pos: BlockPos,
    spot: Spot,
    kind: MoveKind,
    parent: u32,
    g: f32,
    /// Scaffold blocks the path to here places.
    placed: u32,
    /// Index into the search's work list, `NO_WORK` for plain moves.
    work: u32,
}

const NO_WORK: u32 = u32::MAX;

struct Open {
    f: f32,
    g: f32,
    index: u32,
}

impl PartialEq for Open {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Open {}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Open {
    // Min-heap on f; ties go to the deeper node.
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.total_cmp(&self.f).then(self.g.total_cmp(&other.g))
    }
}

/// Node whose spot contains the feet at `feet` (the player may be off-centre, falling or swimming):
/// the feet cell itself, else the nearest valid neighbour cell.
pub fn start_node<B: Blocks>(t: &Terrain<B>, feet: Vec3) -> Option<(BlockPos, Spot)> {
    let [x, z] = [feet[0].floor() as i32, feet[2].floor() as i32];
    let y = (feet[1] - 0.5 - EPS).ceil() as i32;
    if let Some(s) = t.spot([x, y, z]) {
        return Some(([x, y, z], s));
    }
    let mut best: Option<(f32, BlockPos, Spot)> = None;
    for (dx, dy, dz) in (-1..=1).flat_map(|dx| (-2..=1).flat_map(move |dy| (-1..=1).map(move |dz| (dx, dy, dz)))) {
        let p = [x + dx, y + dy, z + dz];
        let Some(s) = t.spot(p) else { continue };
        let d = [p[0] as f32 + 0.5 - feet[0], s.feet - feet[1], p[2] as f32 + 0.5 - feet[2]];
        let d = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if best.is_none_or(|(b, ..)| d < b) {
            best = Some((d, p, s));
        }
    }
    best.map(|(_, p, s)| (p, s))
}

pub fn search<B: Blocks>(t: &Terrain<B>, feet: Vec3, goal: &Goal, opts: &SearchOpts) -> Path {
    let started = Instant::now();
    let empty = |complete| Path { nodes: Vec::new(), complete, cost: 0.0, visited: 0 };
    let Some((start, spot)) = start_node(t, feet) else { return empty(false) };
    if goal.success(start) {
        return empty(true);
    }
    let mut recs = vec![Rec { pos: start, spot, kind: MoveKind::Walk, parent: u32::MAX, g: 0.0, placed: 0, work: NO_WORK }];
    let mut works: Vec<Work> = Vec::new();
    let mut index = PosMap::default();
    index.insert(start, 0u32);
    let mut open = BinaryHeap::from([Open { f: 0.0, g: 0.0, index: 0 }]);
    let mut best = [0u32; 7];
    let mut best_score = [goal.heuristic(start); 7];
    let mut edges: Vec<Edge> = Vec::with_capacity(32);
    let mut visited = 0;
    let mut ctx = Ctx {
        parkour: opts.parkour,
        dig: opts.allow_dig,
        bridge: opts.allow_bridge,
        doors: opts.open_doors,
        kit: &opts.kit,
        blocks_left: 0,
    };

    while let Some(Open { g, index: i, .. }) = open.pop() {
        let rec = &recs[i as usize];
        if g > rec.g {
            continue;
        }
        if goal.success(rec.pos) {
            return Path { nodes: reconstruct(&recs, &works, i), complete: true, cost: g, visited };
        }
        visited += 1;
        if visited >= opts.max_nodes || (visited % 512 == 0 && started.elapsed() >= opts.budget) {
            break;
        }
        edges.clear();
        let placed = rec.placed;
        ctx.blocks_left = opts.kit.blocks.saturating_sub(placed);
        moves::successors(t, rec.pos, rec.spot, &ctx, &mut edges);
        for e in &edges {
            let g2 = g + e.cost;
            let mut next = Rec { pos: e.pos, spot: e.spot, kind: e.kind, parent: i, g: g2, placed: placed + e.work.places(), work: NO_WORK };
            let j = match index.entry(e.pos) {
                Entry::Occupied(o) => {
                    let j = *o.get();
                    if recs[j as usize].g <= g2 {
                        continue;
                    }
                    j
                }
                Entry::Vacant(v) => {
                    let j = recs.len() as u32;
                    recs.push(next);
                    v.insert(j);
                    j
                }
            };
            if !e.work.is_empty() {
                next.work = works.len() as u32;
                works.push(e.work);
            }
            recs[j as usize] = next;
            let h = goal.heuristic(e.pos);
            open.push(Open { f: g2 + h, g: g2, index: j });
            for (k, c) in COEFFICIENTS.iter().enumerate() {
                let score = h + g2 / c;
                if best_score[k] - score > MIN_IMPROVEMENT {
                    (best[k], best_score[k]) = (j, score);
                }
            }
        }
    }
    let pick = best.iter().copied().find(|&b| b != 0).unwrap_or(0);
    let cost = recs[pick as usize].g;
    Path { nodes: reconstruct(&recs, &works, pick), complete: false, cost, visited }
}

fn reconstruct(recs: &[Rec], works: &[Work], mut i: u32) -> Vec<PathNode> {
    let mut nodes = Vec::new();
    while let Some(r) = recs.get(i as usize).filter(|r| r.parent != u32::MAX) {
        let work = works.get(r.work as usize).copied().unwrap_or_default();
        nodes.push(PathNode { pos: r.pos, feet: r.spot.feet, kind: r.kind, wet: r.spot.wet, work });
        i = r.parent;
    }
    nodes.reverse();
    nodes
}
