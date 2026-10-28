//! Rail shapes (`rail_direction` 0..=9): a placed rail joins the rails around it, and those with a free end turn
//! to it; a shape then sticks until a neighbour is placed. Java's `RailState`, as measured on BDS 1.26.52.3
//! (`place_probe SET=redstone`, 2026-10-28): a lone rail is 0 whatever the facing; a straight-only rail joined
//! along both axes runs east-west; with three or four rails beside it a plain rail takes the last curve of NW,
//! NE, SW, SE that fits.

use acacia_physics::BlockPos;

pub const RAILS: [&str; 4] = ["minecraft:rail", "minecraft:golden_rail", "minecraft:detector_rail", "minecraft:activator_rail"];
const NS: u8 = 0;
const EW: u8 = 1;
const SE: u8 = 6;
const SW: u8 = 7;
const NW: u8 = 8;
const NE: u8 = 9;

const N: [i32; 3] = [0, 0, -1];
const S: [i32; 3] = [0, 0, 1];
const W: [i32; 3] = [-1, 0, 0];
const E: [i32; 3] = [1, 0, 0];

/// A rail in a world: (straight only, shape).
pub type Rail = (bool, u8);

pub fn is_rail(name: &str) -> bool {
    RAILS.contains(&name)
}

pub fn straight(name: &str) -> bool {
    name != RAILS[0]
}

fn at(p: BlockPos, d: [i32; 3]) -> BlockPos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

fn up(p: BlockPos) -> BlockPos {
    at(p, [0, 1, 0])
}

/// Where a rail of `shape` at `pos` leads (one up at a rising end).
pub fn ends(pos: BlockPos, shape: u8) -> [BlockPos; 2] {
    let (a, b) = match shape {
        0 => (at(pos, N), at(pos, S)),
        1 => (at(pos, W), at(pos, E)),
        2 => (at(pos, W), up(at(pos, E))),
        3 => (up(at(pos, W)), at(pos, E)),
        4 => (up(at(pos, N)), at(pos, S)),
        5 => (at(pos, N), up(at(pos, S))),
        SE => (at(pos, E), at(pos, S)),
        SW => (at(pos, W), at(pos, S)),
        NW => (at(pos, W), at(pos, N)),
        _ => (at(pos, E), at(pos, N)),
    };
    [a, b]
}

fn leads_to(ends: &[BlockPos], p: BlockPos) -> bool {
    ends.iter().any(|e| e[0] == p[0] && e[2] == p[2])
}

/// A world of rails with placements laid over it.
#[derive(Clone)]
pub struct World<'a> {
    pub base: &'a dyn Fn(BlockPos) -> Option<Rail>,
    over: Vec<(BlockPos, Rail)>,
}

impl<'a> World<'a> {
    pub fn new(base: &'a dyn Fn(BlockPos) -> Option<Rail>) -> Self {
        Self { base, over: Vec::new() }
    }

    pub fn get(&self, p: BlockPos) -> Option<Rail> {
        self.over.iter().rev().find(|(q, _)| *q == p).map(|(_, r)| *r).or_else(|| (self.base)(p))
    }

    fn set(&mut self, p: BlockPos, r: Rail) {
        self.over.push((p, r));
    }

    /// The rail at `p`, or one up or down from it.
    fn near(&self, p: BlockPos) -> Option<BlockPos> {
        [p, up(p), at(p, [0, -1, 0])].into_iter().find(|q| self.get(*q).is_some())
    }

    /// The ends of the rail at `p` that a rail leading back meets (Java `removeSoftConnections`), as rail cells.
    pub fn joined(&self, p: BlockPos) -> Vec<BlockPos> {
        let Some((_, shape)) = self.get(p) else { return Vec::new() };
        ends(p, shape).into_iter().filter_map(|e| self.near(e).filter(|q| self.get(*q).is_some_and(|(_, s)| leads_to(&ends(*q, s), p)))).collect()
    }

    /// The rail beside at `p` would join the rail at `to` (Java `hasNeighborRail`).
    fn joins(&self, p: BlockPos, to: BlockPos) -> bool {
        self.near(p).is_some_and(|q| {
            let j = self.joined(q);
            j.len() != 2 || leads_to(&j, to)
        })
    }

    fn rising(&self, pos: BlockPos, shape: u8) -> u8 {
        let r = |d| self.get(up(at(pos, d))).is_some();
        match shape {
            NS if r(N) => 4,
            NS if r(S) => 5,
            EW if r(E) => 2,
            EW if r(W) => 3,
            s => s,
        }
    }

    fn turned_to(&self, pos: BlockPos, other: BlockPos) -> u8 {
        let Some((straight, _)) = self.get(pos) else { return NS };
        let mut j = self.joined(pos);
        j.push(other);
        let [n, s, w, e] = [N, S, W, E].map(|d| leads_to(&j, at(pos, d)));
        let mut shape = NS;
        if w || e {
            shape = EW;
        }
        if !straight {
            shape = corner(n, s, w, e).unwrap_or(shape);
        }
        self.rising(pos, shape)
    }

    /// Places a rail at `pos` (straight only or not): its shape among the rails around it, then the neighbours at
    /// its ends that can turn to it. Returns every rail whose shape it set, the new one first.
    pub fn place(&mut self, pos: BlockPos, straight: bool) -> Vec<(BlockPos, u8)> {
        self.set(pos, (straight, NS));
        let [n, s, w, e] = [N, S, W, E].map(|d| self.joins(at(pos, d), pos));
        let (z, x) = (n || s, w || e);
        let mut shape = match (z, x) {
            (true, false) => NS,
            (_, true) => EW,
            _ => NS,
        };
        if !straight {
            for (fits, curve) in [(n && w, NW), (n && e, NE), (s && w, SW), (s && e, SE)] {
                if fits {
                    shape = curve;
                }
            }
        }
        let shape = self.rising(pos, shape);
        self.set(pos, (straight, shape));
        let mut out = vec![(pos, shape)];
        for end in ends(pos, shape) {
            let Some(q) = self.near(end) else { continue };
            let j = self.joined(q);
            if j.len() != 2 || leads_to(&j, pos) {
                let turned = self.turned_to(q, pos);
                let st = self.get(q).is_some_and(|r| r.0);
                self.set(q, (st, turned));
                out.push((q, turned));
            }
        }
        out
    }
}

fn corner(n: bool, s: bool, w: bool, e: bool) -> Option<u8> {
    match (n, s, w, e) {
        (false, true, false, true) => Some(SE),
        (false, true, true, false) => Some(SW),
        (true, false, true, false) => Some(NW),
        (true, false, false, true) => Some(NE),
        _ => None,
    }
}

/// `shape` turned `turns` clockwise after an east-west mirror (`transform`).
pub fn turned(shape: u8, turns: u8, mirror: bool) -> u8 {
    // Each shape as its end directions (0 north, 1 east, 2 south, 3 west), the rising end second.
    const DIRS: [(u8, u8, bool); 10] = [(0, 2, false), (3, 1, false), (3, 1, true), (1, 3, true), (2, 0, true), (0, 2, true), (2, 1, false), (2, 3, false), (0, 3, false), (0, 1, false)];
    let turn = |d: u8| {
        let d = if mirror && d % 2 == 1 { 4 - d } else { d };
        (d + turns) % 4
    };
    let (a, b, rise) = DIRS[usize::from(shape.min(9))];
    let (a, b) = (turn(a), turn(b));
    let found = DIRS.iter().position(|&(x, y, r)| r == rise && if rise { (x, y) == (a, b) } else { (x == a && y == b) || (x == b && y == a) });
    found.map_or(shape, |i| i as u8)
}
