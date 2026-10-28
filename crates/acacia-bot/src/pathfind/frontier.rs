//! The A* open set and node records, with lazy leaps: a leap target is queued at a floor on its cost and solved
//! only when popped (docs/pathfind-plan.md "Core fixes").

use std::cmp::Ordering;
use std::collections::hash_map::Entry as Slot;
use std::collections::{BinaryHeap, HashMap};

use super::PathNode;
use super::favour::Favour;
use super::hazard::Hazards;
use super::goal::{Goal, Speed};
use super::leap_moves::{LeapCand, Line};
use super::mac::Macro;
use super::moves::{Edge, MoveKind};
use super::node::{Entry, KeyMap, Momentum, NodeKey};
use super::terrain::Spot;
use super::work::Work;

/// Partial-path candidates are the best nodes by `h + g / c` for each coefficient (Baritone).
const COEFFICIENTS: [f32; 7] = [1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 15.0];
const MIN_IMPROVEMENT: f32 = 0.01;
pub(super) const NONE: u32 = u32::MAX;

#[derive(Clone, Copy)]
pub(super) struct Rec {
    pub key: NodeKey,
    pub spot: Spot,
    pub kind: MoveKind,
    pub parent: u32,
    pub g: f32,
    /// `g` without [`super::SearchOpts::favour`]'s discounts.
    pub raw: f32,
    /// Scaffold blocks the path to here places.
    pub placed: u32,
    /// Index into the search's work list, `NONE` for plain moves.
    work: u32,
    /// Index into the search's macro list, `NONE` for classic moves.
    mac: u32,
    pub mom: Momentum,
}

impl Rec {
    pub fn start(key: NodeKey, spot: Spot, mom: Momentum) -> Self {
        Self { key, spot, kind: MoveKind::Walk, parent: NONE, g: 0.0, raw: 0.0, placed: 0, work: NONE, mac: NONE, mom }
    }
}

struct Open {
    f: f32,
    g: f32,
    index: u32,
    /// Index into the lazy leaps, `NONE` for a node.
    lazy: u32,
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

/// A leap from node `parent`, queued unsolved.
#[derive(Clone, Copy)]
pub(super) struct Lazy {
    pub parent: u32,
    /// The parent's `g` when queued: a parent reached cheaper since queues its leaps again.
    parent_g: f32,
    /// What the leap was queued at: no more than it costs.
    pub floor: f32,
    pub cand: LeapCand,
}

pub(super) enum Popped {
    Node(u32),
    Leap(Lazy),
}

pub(super) struct Frontier<'s> {
    pub recs: Vec<Rec>,
    index: KeyMap<u32>,
    open: BinaryHeap<Open>,
    works: Vec<Work>,
    macs: Vec<Macro>,
    lazies: Vec<Lazy>,
    /// Per (node, line), the fewest steps out a leap fell short at: farther ones on the line would too.
    shorts: HashMap<(u32, Line), i32>,
    pub best: [u32; 7],
    best_score: [f32; 7],
    goal: &'s Goal,
    speed: Speed,
    favour: Option<&'s Favour>,
    hazards: Option<&'s Hazards>,
}

impl<'s> Frontier<'s> {
    pub fn new(start: Rec, goal: &'s Goal, speed: Speed, favour: Option<&'s Favour>, hazards: Option<&'s Hazards>) -> Self {
        let mut index = KeyMap::default();
        index.insert(start.key, 0);
        let h = goal.estimate(start.key.pos, speed, start.key.entry == Entry::Rest);
        let (open, recs) = (BinaryHeap::from([Open { f: 0.0, g: 0.0, index: 0, lazy: NONE }]), vec![start]);
        let (works, macs, lazies, shorts) = Default::default();
        Self { recs, index, open, works, macs, lazies, shorts, best: [0; 7], best_score: [h; 7], goal, speed, favour, hazards }
    }

    /// The next node to expand or leap to solve, skipping those a cheaper path made stale.
    pub fn pop(&mut self) -> Option<Popped> {
        while let Some(Open { g, index, lazy, .. }) = self.open.pop() {
            if lazy == NONE {
                if g <= self.recs[index as usize].g {
                    return Some(Popped::Node(index));
                }
                continue;
            }
            let l = self.lazies[lazy as usize];
            let parent = &self.recs[l.parent as usize];
            let short = self.shorts.get(&(l.parent, l.cand.line(parent.key.pos))).is_some_and(|&k| l.cand.steps() >= k);
            if parent.g == l.parent_g && !short {
                return Some(Popped::Leap(l));
            }
        }
        None
    }

    /// A leap from `l.parent` fell short: the ones as far or farther on its line are skipped.
    pub fn fell_short(&mut self, l: &Lazy) {
        let line = l.cand.line(self.recs[l.parent as usize].key.pos);
        let k = self.shorts.entry((l.parent, line)).or_insert(i32::MAX);
        *k = (*k).min(l.cand.steps());
    }

    /// Queues the leap to `cand` from node `i`, at `floor` (no more than it can cost).
    pub fn queue_leap(&mut self, i: u32, cand: LeapCand, floor: f32) {
        let parent_g = self.recs[i as usize].g;
        let g = parent_g + floor;
        let f = g + self.goal.estimate(cand.q, self.speed, false);
        self.open.push(Open { f, g, index: NONE, lazy: self.lazies.len() as u32 });
        self.lazies.push(Lazy { parent: i, parent_g, floor, cand });
    }

    /// Takes edge `e` from node `i` if it reaches its node cheaper than known.
    pub fn relax(&mut self, i: u32, e: &Edge) {
        let rec = self.recs[i as usize];
        let threat = self.hazards.map_or(0.0, |h| h.edge(rec.key.pos, e.pos));
        let g = rec.g + self.favour.map_or(e.cost, |f| f.cost(rec.key.pos, e.pos, e.cost, self.speed)) + threat;
        let key = NodeKey { pos: e.pos, entry: e.entry, ..rec.key };
        let (placed, raw) = (rec.placed + e.work.places(), rec.raw + e.cost);
        let mut next = Rec { key, spot: e.spot, kind: e.kind, parent: i, g, raw, placed, work: NONE, mac: NONE, mom: e.mom };
        let j = match self.index.entry(key) {
            Slot::Occupied(o) => {
                let j = *o.get();
                if self.recs[j as usize].g <= g {
                    return;
                }
                j
            }
            Slot::Vacant(v) => {
                let j = self.recs.len() as u32;
                self.recs.push(next);
                v.insert(j);
                j
            }
        };
        if !e.work.is_empty() {
            next.work = self.works.len() as u32;
            self.works.push(e.work);
        }
        if let Some(m) = e.mac {
            next.mac = self.macs.len() as u32;
            self.macs.push(m);
        }
        self.recs[j as usize] = next;
        let h = self.goal.estimate(e.pos, self.speed, e.entry == Entry::Rest);
        self.open.push(Open { f: g + h, g, index: j, lazy: NONE });
        for (k, c) in COEFFICIENTS.iter().enumerate() {
            let score = h + g / c;
            if self.best_score[k] - score > MIN_IMPROVEMENT {
                (self.best[k], self.best_score[k]) = (j, score);
            }
        }
    }

    /// The path from the start to node `i`.
    pub fn reconstruct(&self, mut i: u32) -> Vec<PathNode> {
        let mut nodes = Vec::new();
        while let Some(r) = self.recs.get(i as usize).filter(|r| r.parent != NONE) {
            let work = self.works.get(r.work as usize).copied().unwrap_or_default();
            let mac = self.macs.get(r.mac as usize).copied();
            let cost = r.raw - self.recs[r.parent as usize].raw;
            nodes.push(PathNode { pos: r.key.pos, feet: r.spot.feet, kind: r.kind, wet: r.spot.wet, work, entry: r.key.entry, mom: r.mom, mac, cost });
            i = r.parent;
        }
        nodes.reverse();
        nodes
    }
}
