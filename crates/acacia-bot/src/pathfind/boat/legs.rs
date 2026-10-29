//! Boat legs as one search edge, bank to bank (design step 6): each water body met is flooded once, given the
//! landing bank that looks best for the goal, and the boat distance from every water cell to that landing.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::f32::consts::SQRT_2;

use acacia_physics::BlockPos;

use super::grid::{WaterGrid, water_level};
use crate::pathfind::goal::Goal;
use crate::pathfind::moves::{Edge, MoveKind};
use crate::pathfind::node::{Entry, Momentum};
use crate::pathfind::profile::MovementProfile;
use crate::pathfind::terrain::{Blocks, Spot, SpotKind, Terrain};
use crate::pathfind::work::{Step, Work};

/// The fastest a boat goes (cruise 0.378 blocks a tick) over the most the heuristic's octile distance overstates a
/// straight line (8.24% at 22.5°): the heuristic stays admissible over water at this many ticks a block.
pub(crate) const HEURISTIC_FLOOR: f32 = 2.64 / 1.0824;
/// Water cells flooded per body at most; a bigger one is planned over the part flooded.
const MAX_CELLS: usize = 16_384;
/// Boat legs shorter than this (blocks of water) are left to walking and swimming.
const MIN_LEG: f32 = 8.0;
/// Ticks to board (hotbar, place, mount, settle) and to leave (dismount, break, pick up) beside the paddling,
/// [Optimal, Human]: about what the live drills spent besides paddling (2026-10-28).
const OVERHEAD: [f32; 2] = [80.0, 120.0];
/// Planned ticks per block of the water distance, [Optimal, Human]: the sim legs' pace with their turns and stops.
const TICKS_PER_BLOCK: [f32; 2] = [2.7, 3.2];
/// Water a carried boat is placed on from a bank cell: the nearest cells whose boat clears the player.
const LAUNCH: [(i32, i32); 4] = [(2, 0), (-2, 0), (0, 2), (0, -2)];
/// A seen boat this near a bank cell's centre (blocks) is boarded from it.
const BOARD_REACH: f32 = 2.5;

/// What a boat leg can board; [`goto`](crate::Bot::goto) fills it from the inventory and the entities in sight.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoatSource {
    /// A boat item in the inventory.
    pub carried: bool,
    /// Empty boats in sight (positions).
    pub seen: Vec<[f32; 3]>,
}

impl BoatSource {
    pub fn any(&self) -> bool {
        self.carried || !self.seen.is_empty()
    }
}

struct Body {
    /// The bank cell to land on and its spot.
    landing: Option<(BlockPos, Spot)>,
    /// Boat distance (blocks) from each water cell (x, z) to the landing's water.
    dist: HashMap<(i32, i32), f32>,
}

/// The boat legs one search may plan.
pub(crate) struct BoatLegs {
    goal: Goal,
    human: bool,
    source: BoatSource,
    bodies: RefCell<Vec<Body>>,
    /// Water cell to its body.
    of: RefCell<HashMap<BlockPos, usize>>,
}

impl BoatLegs {
    /// `None` with nothing to board.
    pub fn new(goal: &Goal, profile: MovementProfile, source: &BoatSource) -> Option<Self> {
        source.any().then(|| Self {
            goal: goal.clone(),
            human: profile == MovementProfile::Human,
            source: source.clone(),
            bodies: RefCell::default(),
            of: RefCell::default(),
        })
    }

    /// The boat leg from bank cell `p`, if any: on dry ground beside water a boat floats on. A run into `p` counts
    /// too: the follower stops there for the leg's work.
    pub fn edges<B: Blocks>(&self, t: &Terrain<B>, p: BlockPos, from: Spot, entry: Entry, out: &mut Vec<Edge>) {
        if entry == Entry::Air || from.kind != SpotKind::Stand || from.wet || from.made != 0 {
            return;
        }
        let y = p[1] - 1;
        let grid = WaterGrid::new(t, y);
        let i = usize::from(self.human);
        let mut best: Option<(f32, BlockPos, Spot)> = None;
        for w in self.launches(&grid, p) {
            let Some((dist, landing, spot)) = self.leg(t, w, y) else { continue };
            let cost = OVERHEAD[i] + dist * TICKS_PER_BLOCK[i];
            if dist >= MIN_LEG && best.is_none_or(|(c, ..)| cost < c) {
                best = Some((cost, landing, spot));
            }
        }
        if let Some((cost, pos, spot)) = best {
            let mut work = Work::default();
            work.push(Step::Boat);
            out.push(Edge { pos, spot, kind: MoveKind::Boat, cost, work, entry: Entry::Rest, mom: Momentum::rest(spot.feet, pos), mac: None });
        }
    }

    /// Water cells (x, z) a boat is boarded on from `p`: placed from the hotbar, else a seen boat's.
    fn launches<B: Blocks>(&self, grid: &WaterGrid<'_, Terrain<'_, B>>, p: BlockPos) -> Vec<[i32; 2]> {
        if self.source.carried {
            let clear = |w: &[i32; 2]| grid.clear(w[0] as f32 + 0.5, w[1] as f32 + 0.5, 0.0);
            return LAUNCH.iter().map(|(dx, dz)| [p[0] + dx, p[2] + dz]).filter(clear).collect();
        }
        let centre = [p[0] as f32 + 0.5, p[2] as f32 + 0.5];
        self.source.seen.iter().filter(|b| water_level(b[1]) == grid.y && (b[0] - centre[0]).hypot(b[2] - centre[1]) <= BOARD_REACH)
            .map(|b| [b[0].floor() as i32, b[2].floor() as i32]).collect()
    }

    /// (water distance, landing, its spot) from water cell `w` at level `y`.
    fn leg<B: Blocks>(&self, t: &Terrain<B>, w: [i32; 2], y: i32) -> Option<(f32, BlockPos, Spot)> {
        let key = [w[0], y, w[1]];
        let known = self.of.borrow().get(&key).copied();
        let i = match known {
            Some(i) => i,
            None => self.flood(t, key)?,
        };
        let bodies = self.bodies.borrow();
        let (landing, spot) = bodies[i].landing?;
        Some((*bodies[i].dist.get(&(w[0], w[1]))?, landing, spot))
    }

    /// Floods the body of water cell `start` (cells a boat can be centred in), picks its landing and measures it.
    fn flood<B: Blocks>(&self, t: &Terrain<B>, start: BlockPos) -> Option<usize> {
        let y = start[1];
        let grid = WaterGrid::new(t, y);
        let centred = |x: i32, z: i32| grid.clear(x as f32 + 0.5, z as f32 + 0.5, 0.0);
        if !centred(start[0], start[2]) {
            return None;
        }
        let mut cells = vec![(start[0], start[2])];
        let mut seen = HashSet::from([(start[0], start[2])]);
        let mut queue = VecDeque::from([(start[0], start[2])]);
        // (score, landing, spot, its water cell)
        let mut landing: Option<(f32, BlockPos, Spot, (i32, i32))> = None;
        while let Some((x, z)) = queue.pop_front() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (x + dx, z + dz);
                if !seen.contains(&n) && cells.len() < MAX_CELLS && centred(n.0, n.1) {
                    seen.insert(n);
                    cells.push(n);
                    queue.push_back(n);
                }
                // A centred boat clears a block either side: its bank is two out.
                let bank = [x + 2 * dx, y + 1, z + 2 * dz];
                if grid.floats(bank[0], bank[2]) {
                    continue;
                }
                let Some(spot) = t.spot(bank).filter(|s| s.kind == SpotKind::Stand && !s.wet && s.feet == bank[1] as f32) else { continue };
                let score = self.goal.heuristic(bank);
                if landing.is_none_or(|(s, ..)| score < s) {
                    landing = Some((score, bank, spot, (x, z)));
                }
            }
        }
        let dist = landing.map(|(.., from)| distances(&seen, from)).unwrap_or_default();
        let mut bodies = self.bodies.borrow_mut();
        let i = bodies.len();
        bodies.push(Body { landing: landing.map(|(_, b, s, _)| (b, s)), dist });
        self.of.borrow_mut().extend(cells.into_iter().map(|(x, z)| ([x, y, z], i)));
        Some(i)
    }
}

/// Dijkstra over `cells` from `from`, eight ways (a diagonal only past two water cells).
fn distances(cells: &HashSet<(i32, i32)>, from: (i32, i32)) -> HashMap<(i32, i32), f32> {
    let mut dist = HashMap::from([(from, 0.0)]);
    let mut heap = BinaryHeap::from([Near(0.0, from)]);
    while let Some(Near(d, (x, z))) = heap.pop() {
        if dist.get(&(x, z)).is_some_and(|&b| d > b) {
            continue;
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                let n = (x + dx, z + dz);
                let diagonal = dx != 0 && dz != 0;
                if (dx, dz) == (0, 0) || !cells.contains(&n) || (diagonal && !(cells.contains(&(x + dx, z)) && cells.contains(&(x, z + dz)))) {
                    continue;
                }
                let nd = d + if diagonal { SQRT_2 } else { 1.0 };
                if dist.get(&n).is_none_or(|&b| nd < b) {
                    dist.insert(n, nd);
                    heap.push(Near(nd, n));
                }
            }
        }
    }
    dist
}

/// A heap entry, nearest first.
struct Near(f32, (i32, i32));

impl PartialEq for Near {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Near {}

impl PartialOrd for Near {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Near {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0)
    }
}
