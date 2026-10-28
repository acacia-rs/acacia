//! A* over feet block positions with Baritone's best-partial-path fallback (azalea `astar/mod.rs`).

use std::cell::Cell;
use std::time::Instant;

use acacia_physics::{BlockPos, PlayerState, Vec3};

use super::boat::BoatLegs;
use super::cache::RolloutCache;
use super::exit::{Exits, has_exit};
use super::frontier::{Frontier, Popped, Rec};
use super::goal::Goal;
use super::macros::{self, MACRO_TIME_SHARE, MacroStats, Macros};
use super::costs::CLASSIC_STARTUP;
use super::moves::{self, Ctx, Edge, MoveKind};
use super::node::{Entry, Momentum, NodeKey, run_entry};
pub use super::opts::{Path, SearchOpts};
use super::reach::{self, Reaches};
use super::solve::Miss;
use super::terrain::{Blocks, EPS, Spot, Terrain};

/// Slower than this (blocks per tick) on the ground counts as standing.
const AT_REST: f32 = 0.05;

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

/// Searches from a player standing at `feet`.
pub fn search<B: Blocks>(t: &Terrain<B>, feet: Vec3, goal: &Goal, opts: &SearchOpts) -> Path {
    let mut st = PlayerState::new(feet);
    st.on_ground = true;
    search_from(t, &st, goal, opts)
}

/// Searches from the player's live state: its momentum is the start node's, and its attributes and
/// effects shape every macro move.
pub fn search_from<B: Blocks>(t: &Terrain<B>, live: &PlayerState, goal: &Goal, opts: &SearchOpts) -> Path {
    search_cached(t, live, goal, opts, &RolloutCache::default())
}

/// [`search_from`] reusing (and adding to) the macro-move results in `cache`; they are dropped first if
/// they were found for another profile or player.
pub fn search_cached<B: Blocks>(t: &Terrain<B>, live: &PlayerState, goal: &Goal, opts: &SearchOpts, cache: &RolloutCache) -> Path {
    let started = Instant::now();
    let empty = |complete| Path { nodes: Vec::new(), complete, unreachable: false, cost: 0.0, visited: 0, macro_stats: MacroStats::default() };
    let Some((start, spot)) = start_node(t, live.pos) else { return empty(false) };
    if goal.success(start) {
        return empty(true);
    }
    let entry = match live.on_ground {
        false => Entry::Air,
        true if live.vel[0].hypot(live.vel[2]) < AT_REST => Entry::Rest,
        true => run_entry(live.vel),
    };
    let start = NodeKey { entry, ..NodeKey::rest(start) };
    let speed = opts.speed();
    let mut front = Frontier::new(Rec::start(start, spot, Momentum::snap(live, start.pos)), goal, speed, opts.favour.as_deref());
    let mut edges: Vec<Edge> = Vec::with_capacity(32);
    let (mut visited, mut leaps) = (0, 0usize);
    let macros = opts.macros.then(|| Macros {
        profile: opts.profile,
        leaps: opts.parkour,
        hops: opts.hops,
        base: live,
        cache: cache.claim(opts.profile, live),
        budget: opts.max_rollout_ticks,
        deadline: started + opts.budget / MACRO_TIME_SHARE,
        stats: Cell::default(),
    });
    let boats = opts.boats.then(|| BoatLegs::new(goal, opts.profile, &opts.boat_source)).flatten();
    let mut ctx = Ctx { parkour: opts.parkour, dig: opts.allow_dig, bridge: opts.allow_bridge, doors: opts.open_doors, kit: &opts.kit, blocks_left: 0, macros, keep: opts.keep.as_deref(), no_body: opts.no_body.as_deref(), boats: boats.as_ref() };
    let stats = |ctx: &Ctx| ctx.macros.as_ref().map_or_else(MacroStats::default, |m| m.stats.get());
    let (mut cut, mut reaches, mut exits) = (false, Reaches::default(), Exits::default());

    while let Some(popped) = front.pop() {
        let i = match popped {
            Popped::Node(i) => i,
            Popped::Leap(l) => {
                leaps += 1;
                if leaps % 8 == 0 && started.elapsed() >= opts.budget {
                    cut = true;
                    break;
                }
                let (Some(m), r) = (&ctx.macros, front.recs[l.parent as usize]) else { continue };
                // From the search's start, the bot's real facing (it may face away); other nodes face their leap.
                match macros::leap(t, r.key.pos, &r.mom, l.cand, l.parent == 0, m) {
                    Ok(e) => {
                        debug_assert!(e.cost >= l.floor - 1e-3, "leap {:?} -> {:?} cost {} under its floor {}", r.key.pos, e.pos, e.cost, l.floor);
                        front.relax(l.parent, &e)
                    }
                    Err(Miss::Short) => front.fell_short(&l),
                    Err(Miss::Other) => {}
                }
                continue;
            }
        };
        let rec = front.recs[i as usize];
        if goal.success(rec.key.pos) {
            return Path { nodes: front.reconstruct(i), complete: true, unreachable: false, cost: rec.raw, visited, macro_stats: stats(&ctx) };
        }
        visited += 1;
        // Checked often: a node with macro moves costs many rollouts.
        if visited >= opts.max_nodes || (visited % 8 == 0 && started.elapsed() >= opts.budget) {
            cut = true;
            break;
        }
        edges.clear();
        ctx.blocks_left = opts.kit.blocks.saturating_sub(rec.placed);
        moves::successors(t, rec.key.pos, rec.spot, (rec.key.entry, &rec.mom), &ctx, &mut edges);
        if i == 0 && live.vel[0].hypot(live.vel[2]) < AT_REST {
            // Classic moves from standing pay their run-up, as the simulated macro moves beside them do.
            let walks = |e: &&mut Edge| e.mac.is_none() && e.work.is_empty() && !e.spot.wet && matches!(e.kind, MoveKind::Walk | MoveKind::Ascend | MoveKind::Descend | MoveKind::Parkour);
            edges.iter_mut().filter(walks).for_each(|e| e.cost += CLASSIC_STARTUP);
        }
        edges.iter().for_each(|e| front.relax(i, e));
        if let Some(m) = &ctx.macros {
            for c in macros::leap_candidates(t, rec.key.pos, rec.spot, (rec.key.entry, &rec.mom), m) {
                // Down into a basin with no way on (a pit floor): never on a path to the goal.
                if c.q[1] < rec.key.pos[1] && !c.to.wet &&!exits.open(t, c.q, c.to, opts, goal) {
                    continue;
                }
                let reach = reaches.get(&rec.mom, live);
                front.queue_leap(i, c, reach::leap_floor(t, rec.key.pos, &rec.mom, &c, reach, live, opts.profile));
            }
        }
    }
    let macro_stats = stats(&ctx);
    let unreachable = !cut && !macro_stats.exhausted && !t.saw_unloaded();
    let mut pick = front.best.iter().copied().find(|&b| b != 0).unwrap_or(0);
    // Back off a dead end to the last node on the way that still leads somewhere.
    while pick != 0 {
        let r = front.recs[pick as usize];
        if has_exit(t, r.key.pos, r.spot, opts, r.placed) {
            break;
        }
        pick = r.parent;
    }
    let cost = front.recs[pick as usize].raw;
    Path { nodes: front.reconstruct(pick), complete: false, unreachable, cost, visited, macro_stats }
}
