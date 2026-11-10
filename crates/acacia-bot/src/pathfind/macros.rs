//! Macro successors: moves found by simulating their controller from the node's momentum
//! (`docs/pathfind-plan.md` "Macro moves"), cached across nodes ([`RolloutCache`]) and capped by a rollout
//! tick budget.

use std::cell::Cell;
use std::sync::Arc;
use std::time::Instant;

use acacia_physics::BlockPos;

use super::cache::{Counts, Store};
use super::cell::AVOID;
use super::costs::{DANGER_NEAR, HUMAN_JUMP, RISK, RISK_MARGIN};
use super::hop;
use super::leap::JumpAt;
use super::leap_moves::{self, LeapCand};
use super::mac::Macro;
use super::moves::{Edge, MoveKind};
use super::node::{Entry, Momentum, run_entry};
use super::profile::MovementProfile;
use super::solve::{Miss, Solved, Solver};
use super::terrain::{Blocks, Spot, SpotKind, Terrain};
use super::rollout::PlanSim;
use crate::movement::{Controls, Motion, Sim};

/// Macro rollouts may take this share (1/n) of a search's time budget. The default tick budget alone takes about the
/// whole budget, so a slowed search ran out of time just after it, and the partial path it fell back to led into the
/// course's pits (docs/pathfind-plan.md step 10).
pub(crate) const MACRO_TIME_SHARE: u32 = 2;

/// What one search spent on macro moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MacroStats {
    pub rollouts: u32,
    /// Ticks simulated.
    pub ticks: u64,
    /// Move families answered by the cache, and those simulated.
    pub hits: u32,
    pub misses: u32,
    /// Cached results dropped because their key was full.
    pub evicted: u32,
    /// The tick budget ran out: later nodes got classic moves only.
    pub exhausted: bool,
}

/// What the search's macro moves need.
pub(crate) struct Macros<'a, S> {
    pub profile: MovementProfile,
    /// Gap leaps (`SearchOpts::parkour`).
    pub leaps: bool,
    /// Sprint-jumps along flat ground (`SearchOpts::hops`).
    pub hops: bool,
    /// The live state: attributes, effects and equipment of every rollout.
    pub base: &'a S,
    pub cache: Arc<Store>,
    /// Rollout ticks the search may simulate (`SearchOpts::max_rollout_ticks`).
    pub budget: u64,
    /// No rollouts after this: the rest of the search's time is for classic moves (see [`MACRO_TIME_SHARE`]).
    pub deadline: Instant,
    pub stats: Cell<MacroStats>,
}

/// Macro moves start standing dry on a node the search did not change.
fn can_start(entry: Entry, from: Spot) -> bool {
    entry != Entry::Air && from.kind == SpotKind::Stand && !from.wet && from.made == 0
}

/// Runs `f` with a solver from `mom` at `p` and books what it simulated; `None` once the budget or time is spent.
fn simulate<B: Blocks, S: PlanSim<B>, R>(t: &Terrain<B>, p: BlockPos, mom: &Momentum, m: &Macros<S>, f: impl FnOnce(&S, &mut Solver<Terrain<B>>, (&Store, &mut Counts)) -> R) -> Option<R> {
    let mut stats = m.stats.get();
    if stats.ticks >= m.budget || Instant::now() >= m.deadline {
        stats.exhausted = true;
        m.stats.set(stats);
        return None;
    }
    let start = mom.synth(p, m.base);
    let avoid = |q: BlockPos| t.cell(q).has(AVOID);
    let safe = |q: BlockPos| t.spot(q).is_some_and(|s| s.kind == SpotKind::Stand && !s.wet && s.feet == q[1] as f32);
    let mut solver = Solver::new(t, &avoid, &safe, m.profile, true, Controls::default());
    solver.budget = m.budget - stats.ticks;
    let mut n = Counts { off: m.cache.off, ..Counts::default() };
    let r = f(&start, &mut solver, (&m.cache, &mut n));
    stats.rollouts += solver.rollouts;
    stats.ticks += solver.ticks;
    stats.hits += n.hits;
    stats.misses += n.misses;
    stats.evicted += n.evicted;
    stats.exhausted |= solver.spent();
    m.stats.set(stats);
    Some(r)
}

/// Runs and hops from `p`; leaps are offered lazily ([`leap_candidates`], [`leap`]).
pub(crate) fn successors<B: Blocks, S: PlanSim<B>>(t: &Terrain<B>, p: BlockPos, from: Spot, (entry, mom): (Entry, &Momentum), m: &Macros<S>, out: &mut Vec<Edge>) {
    if !can_start(entry, from) {
        return;
    }
    simulate(t, p, mom, m, |start, solver, (cache, n)| {
        for (q, to, r) in hop::runs(t, p, from, (entry, mom), start, solver, (cache, &mut *n)) {
            push(t, out, q, to, MoveKind::Run, r.ticks as f32, &r.end, Macro::Run(r.ctl));
        }
        if m.hops
            && !solver.spent()
            && let Entry::Run(d, _) = entry
            && let Some((q, to, s)) = cache.hops.get_or(t, p, (*mom, d), n, || hop::hop(t, p, from, d, start, solver))
        {
            push_leap(t, out, q, to, s, MoveKind::Hop, m.profile);
        }
    });
}

/// Leap targets from `p`, to be solved when the search reaches them.
pub(crate) fn leap_candidates<B: Blocks, S: Sim>(t: &Terrain<B>, p: BlockPos, from: Spot, (entry, mom): (Entry, &Momentum), m: &Macros<S>) -> Vec<LeapCand> {
    let stats = m.stats.get();
    if !m.leaps || !can_start(entry, from) || stats.exhausted {
        return Vec::new();
    }
    leap_moves::candidates(t, p, from, &mom.synth(p, m.base))
}

/// The leap from `p` to `c` as an edge, if it lands; `Err(Miss::Other)` once the budget is spent.
/// `real_yaw`: keep the momentum's facing (the live start) rather than face the leap.
pub(crate) fn leap<B: Blocks, S: PlanSim<B>>(t: &Terrain<B>, p: BlockPos, mom: &Momentum, c: LeapCand, real_yaw: bool, m: &Macros<S>) -> Result<Edge, Miss> {
    let solved = simulate(t, p, mom, m, |start, solver, cache| leap_moves::solve(t, p, (mom, real_yaw), start, c, solver, cache)).unwrap_or(Err(Miss::Other))?;
    let mut out = Vec::with_capacity(1);
    push_leap(t, &mut out, c.q, c.to, solved, MoveKind::Parkour, m.profile);
    Ok(out.remove(0))
}

fn push_leap<B: Blocks>(t: &Terrain<B>, out: &mut Vec<Edge>, pos: BlockPos, to: Spot, s: Solved, kind: MoveKind, profile: MovementProfile) {
    let mut cost = s.ticks as f32;
    if profile == MovementProfile::Human {
        cost += RISK * (1.0 - s.margin / RISK_MARGIN).max(0.0) + if s.ctl.jump == JumpAt::Never { 0.0 } else { HUMAN_JUMP };
    }
    push(t, out, pos, to, kind, cost, &s.end, Macro::Leap(s.ctl));
}

#[allow(clippy::too_many_arguments)]
fn push<B: Blocks>(t: &Terrain<B>, out: &mut Vec<Edge>, pos: BlockPos, to: Spot, kind: MoveKind, mut cost: f32, end: &Motion, mac: Macro) {
    if t.danger_near(pos) {
        cost += DANGER_NEAR;
    }
    let (entry, mom) = if to.wet { (Entry::Rest, Momentum::rest(to.feet, pos)) } else { (run_entry(end.vel), Momentum::snap(end, pos)) };
    out.push(Edge { pos, spot: to, kind, cost, work: Default::default(), entry, mom, mac: Some(mac) });
}
