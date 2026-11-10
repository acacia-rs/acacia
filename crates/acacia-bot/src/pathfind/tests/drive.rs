//! Following a path in the headless simulation the way the navigator does: macro moves run live, and a
//! divergence re-plans from the live state without stopping. Every run is checked against the follower's
//! per-tick rollout budget.

use std::time::{Duration, Instant};

use acacia_physics::{BlockPos, Vec3};

use super::grid::Grid;
use super::sim::sense;
use crate::movement::{Movement, Rollout};
use crate::pathfind::{FollowStatus, Follower, Goal, MacroReport, MovementProfile, Path, PathNode, ROLLOUT_BUDGET, SearchOpts, search_from};

pub const PROFILES: [MovementProfile; 2] = [MovementProfile::Optimal, MovementProfile::Human];

/// Execution aim seeds: `Optimal` has no jitter and runs once, `Human` runs under five streams.
pub fn seeds(profile: MovementProfile) -> &'static [u64] {
    match profile {
        MovementProfile::Optimal => &[0],
        MovementProfile::Human => &[1, 2, 3, 4, 5],
    }
}

pub fn macro_opts(profile: MovementProfile) -> SearchOpts {
    // Bounded by nodes, not time: debug builds simulate slowly, and a timed-out plan would be a flaky one.
    SearchOpts { max_nodes: 20_000, budget: Duration::from_secs(3600), profile, ..SearchOpts::default() }
}

/// A movement standing at `feet`, facing +x.
pub fn standing(feet: Vec3) -> Movement {
    let mut m = Movement::new();
    m.start(feet, -90.0, 0.0);
    m
}

/// Searches from `m`'s live state, logging the search's size.
pub fn plan(g: &Grid, m: &Movement, goal: &Goal, opts: &SearchOpts) -> Path {
    let t0 = Instant::now();
    let p = search_from(&g.terrain(), &Rollout::new(m.state().expect("started").clone()), goal, opts);
    let ms = t0.elapsed().as_secs_f32() * 1000.0;
    eprintln!("plan {:?}: {} nodes, {} visited, {ms:.1} ms, complete {}, cost {:.1}, {:?}", opts.profile, p.nodes.len(), p.visited, p.complete, p.cost, p.macro_stats);
    if std::env::var("PLAN_NODES").is_ok() {
        p.nodes.iter().for_each(|n| eprintln!("  NODE {:?} {:?} {:?} cost {:.1}", n.pos, n.kind, n.entry, n.cost));
    }
    p
}

/// How the follower runs: its aim seed, and whether it takes shortcuts.
#[derive(Debug, Clone, Copy, Default)]
pub struct Style {
    pub seed: u64,
    pub shortcuts: bool,
}

pub struct Outcome {
    pub reports: Vec<MacroReport>,
    pub diverged: u32,
    /// Forward was still held on every tick that diverged.
    pub held: bool,
    pub ticks: u32,
    pub end: Vec3,
    /// Most rollout ticks one follower tick simulated.
    pub max_spend: u64,
    /// First tick the feet were within the arrival tolerance of the goal's centre (before settling there).
    pub near: Option<u32>,
}

/// Follows `path` from `m` towards `goal`; `nudge` may disturb the bot each tick. Asserts no follower tick
/// simulates more than [`ROLLOUT_BUDGET`].
pub fn drive(g: &Grid, m: &mut Movement, path: Vec<PathNode>, goal: &Goal, opts: &SearchOpts, style: Style, mut nudge: impl FnMut(u32, &mut Movement)) -> Outcome {
    let world = g.physics();
    let start = m.position().expect("started");
    let mut rng = style.seed;
    let mut new_follower = |path, at| Follower::new(path, at, 0.25).moving(opts.profile, crate::cadence::splitmix(&mut rng)).shortcuts(style.shortcuts);
    let mut follower = new_follower(path, start);
    let mut out = Outcome { reports: Vec::new(), diverged: 0, held: true, ticks: 0, end: start, max_spend: 0, near: None };
    let centre = goal_centre(goal);
    for tick in 0..600 {
        nudge(tick, m);
        let s = sense(g, m);
        if out.near.is_none() && centre.is_some_and(|c| (s.pos[0] - c[0]).hypot(s.pos[2] - c[1]) < 0.25) {
            out.near = Some(tick);
        }
        let live = m.fork();
        let status = follower.tick(&s, live.as_ref(), &g.terrain(), &mut m.controls);
        let spend = follower.spend();
        if std::env::var("DRIVE_TRACE").is_ok() {
            eprintln!("DT {tick} {:.3?} index {} yaw {:.1}", s.pos, follower.index(), m.controls.yaw);
        }
        assert!(spend.ticks <= ROLLOUT_BUDGET, "tick {tick}: {spend:?}");
        out.max_spend = out.max_spend.max(spend.ticks);
        out.reports.extend(follower.take_macro_report());
        match status {
            FollowStatus::Moving => {}
            FollowStatus::Arrived => {
                eprintln!("SPEND arrived after {tick} ticks, {} diverged, max follower spend {} rollout ticks", out.diverged, out.max_spend);
                (out.end, out.ticks) = (s.pos, tick);
                return out;
            }
            FollowStatus::Diverged => {
                eprintln!("tick {tick}: diverged at {:?}, controls {:?}, {spend:?}", s.pos, m.controls);
                out.diverged += 1;
                out.held &= m.controls.forward != 0.0;
                let path = plan(g, m, goal, opts);
                assert!(path.complete, "no path from {:?}", s.pos);
                follower = new_follower(path.nodes, s.pos);
            }
            other => panic!("tick {tick}: {other:?} at {:?}, next {:?}", s.pos, follower.ahead().first()),
        }
        m.tick(&world);
    }
    panic!("not arrived: at {:?}, next {:?}", m.position(), follower.ahead().first());
}

/// Plans from standing at `from` and follows the plan once per seed of the profile, asserting arrival with
/// no divergence and, for `Optimal`, every macro move exactly as predicted. `None` without a complete plan.
pub fn plan_and_follow(g: &Grid, from: Vec3, goal: BlockPos, opts: &SearchOpts, shortcuts: bool) -> Option<(Path, Vec<Outcome>)> {
    let to = Goal::Block(goal);
    let path = plan(g, &standing(from), &to, opts);
    if !path.complete {
        return None;
    }
    let runs: Vec<Outcome> = seeds(opts.profile).iter().map(|&seed| {
        let o = drive(g, &mut standing(from), path.nodes.clone(), &to, opts, Style { seed, shortcuts }, |_, _| {});
        eprintln!("{:?} seed {seed} to {goal:?}: {} ticks, max spend {}, reports {:?}", opts.profile, o.ticks, o.max_spend, o.reports);
        assert_arrived(&o, goal);
        assert_eq!(o.diverged, 0, "{:?} seed {seed}", opts.profile);
        if opts.profile == MovementProfile::Optimal {
            assert_exact(&o);
        }
        o
    }).collect();
    Some((path, runs))
}

fn goal_centre(goal: &Goal) -> Option<[f32; 2]> {
    match goal {
        Goal::Block(p) => Some([p[0] as f32 + 0.5, p[2] as f32 + 0.5]),
        _ => None,
    }
}

pub fn assert_arrived(o: &Outcome, goal: BlockPos) {
    let d = (o.end[0] - goal[0] as f32 - 0.5).hypot(o.end[2] - goal[2] as f32 - 0.5);
    assert!(d < 0.3 && (o.end[1] - goal[1] as f32).abs() < 0.1, "ended at {:?}, wanted {goal:?}", o.end);
}

/// Every macro move ran exactly as predicted when it started (the `Optimal` invariant).
pub fn assert_exact(o: &Outcome) {
    for r in &o.reports {
        assert_eq!((r.executed, r.max_error, r.repredicted), (r.predicted, 0.0, 0), "{:?}", o.reports);
    }
}

/// The path's macro-move kinds and landing cells.
pub fn macro_nodes(p: &Path) -> Vec<(crate::pathfind::MoveKind, BlockPos)> {
    p.nodes.iter().filter(|n| n.mac.is_some()).map(|n| (n.kind, n.pos)).collect()
}
