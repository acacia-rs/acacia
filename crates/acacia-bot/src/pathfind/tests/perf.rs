//! Planner cost per case (plan step 7). `perf_report` is for release builds:
//! `cargo test --release -p acacia-bot --lib perf_report -- --ignored --nocapture`.

use std::time::Instant;

use acacia_physics::Vec3;

use super::drive::{macro_opts, standing};
use super::grid::Grid;
use super::leaps::gap_course;
use crate::movement::Rollout;
use crate::pathfind::{Goal, MovementProfile, Path, RolloutCache, SearchOpts, search_cached};

const OPTIMAL: MovementProfile = MovementProfile::Optimal;
const HUMAN: MovementProfile = MovementProfile::Human;

pub struct Case {
    pub name: &'static str,
    pub grid: fn() -> Grid,
    pub from: Vec3,
    pub goal: Goal,
    pub opts: SearchOpts,
}

fn flat() -> Grid {
    Grid::floor(-32, 47)
}

fn gap3() -> Grid {
    gap_course(3)
}

fn gap5() -> Grid {
    gap_course(5)
}

pub fn cases() -> Vec<Case> {
    let off = SearchOpts { macros: false, ..macro_opts(OPTIMAL) };
    let case = |name, grid, from, goal, opts| Case { name, grid, from, goal, opts };
    vec![
        case("gap course Optimal", gap3 as fn() -> Grid, [-8.5, 1.0, 0.5], Goal::Block([10, 1, 0]), macro_opts(OPTIMAL)),
        case("gap course Human", gap3, [-8.5, 1.0, 0.5], Goal::Block([10, 1, 0]), macro_opts(HUMAN)),
        case("flat 80 macros off", flat, [-31.5, 1.0, 0.5], Goal::Block([47, 1, 0]), off),
        case("flat 80 Optimal", flat, [-31.5, 1.0, 0.5], Goal::Block([47, 1, 0]), macro_opts(OPTIMAL)),
        case("flat 80 Human", flat, [-31.5, 1.0, 0.5], Goal::Block([47, 1, 0]), macro_opts(HUMAN)),
        case("diagonal 30x11 Optimal", flat, [-15.5, 1.0, -5.5], Goal::Block([15, 1, 6]), macro_opts(OPTIMAL)),
        case("diagonal 30x11 Human", flat, [-15.5, 1.0, -5.5], Goal::Block([15, 1, 6]), macro_opts(HUMAN)),
        case("5-gap refusal Human", gap5, [0.5, 1.0, 0.5], Goal::Block([9, 1, 0]), macro_opts(HUMAN)),
    ]
}

/// Plans case `c` with `cache`: the path and milliseconds taken.
pub fn run(c: &Case, g: &Grid, cache: &RolloutCache) -> (Path, f32) {
    let m = standing(c.from);
    let t0 = Instant::now();
    let p = search_cached(&g.terrain(), &Rollout::new(m.state().expect("started").clone()),&c.goal, &c.opts, cache);
    (p, t0.elapsed().as_secs_f32() * 1000.0)
}

fn report(name: &str, mode: &str, (p, ms): &(Path, f32)) {
    let s = p.macro_stats;
    let rate = 100.0 * s.hits as f32 / (s.hits + s.misses).max(1) as f32;
    eprintln!(
        "PERF {name:<24} {mode:<8} {ms:>8.1} ms  visited {:>6}  rollouts {:>6}  ticks {:>8}  hits {rate:>3.0}%  complete {}  cost {:.0}  macro nodes {}",
        p.visited,
        s.rollouts,
        s.ticks,
        p.complete,
        p.cost,
        p.nodes.iter().filter(|n| n.mac.is_some()).count()
    );
}

/// Planned cost against executed ticks, classic and macro plans alike (plan step 7: classic costs ignore
/// acceleration).
#[test]
#[ignore = "diagnostic"]
fn cost_skew_report() {
    let g = flat();
    for profile in [OPTIMAL, HUMAN] {
        for macros in [false, true] {
            // Away from the grid's edge: physics freezes a body near unloaded chunks.
            for to in [-16, -11, -1, 19, 39] {
                let goal = Goal::Block([to, 1, 0]);
                let opts = SearchOpts { macros, ..macro_opts(profile) };
                let mut m = standing([-20.5, 1.0, 0.5]);
                let p = super::drive::plan(&g, &m, &goal, &opts);
                let o = super::drive::drive(&g, &mut m, p.nodes.clone(), &goal, &opts, super::drive::Style::default(), |_, _| {});
                eprintln!("STAT {profile:?} macros {macros} {} blocks: cost {:.1}, near the goal after {:?}, executed {} ticks", to + 21, p.cost, o.near, o.ticks);
            }
        }
    }
}

#[test]
#[ignore = "timings; run in release"]
fn perf_report() {
    // PERF_CASE=<name substring> runs only the matching cases; PERF_UNCACHED=1 adds runs without the cache.
    let only = std::env::var("PERF_CASE").unwrap_or_default();
    let uncached = std::env::var("PERF_UNCACHED").is_ok();
    for c in cases().into_iter().filter(|c| c.name.contains(&only)) {
        let g = (c.grid)();
        if uncached {
            report(c.name, "uncached", &run(&c, &g, &RolloutCache::disabled()));
        }
        // Cold: a fresh grid (terrain memo) and cache; replan: the navigator's cache kept. Best of five (a
        // shared testbox is noisy).
        let best = |runs: Vec<(Path, f32)>| runs.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).expect("runs");
        report(c.name, "cold", &best((0..5).map(|_| run(&c, &(c.grid)(), &RolloutCache::default())).collect()));
        let cache = RolloutCache::default();
        run(&c, &g, &cache);
        report(c.name, "replan", &best((0..5).map(|_| run(&c, &(c.grid)(), &cache)).collect()));
    }
}
