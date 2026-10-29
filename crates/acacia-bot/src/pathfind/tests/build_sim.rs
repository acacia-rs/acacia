//! The builder in the headless simulation: the grid stands in for the server (placed states from
//! `place_model`), acts take the ticks they take live, and with `delay` a bot sees other bots' blocks late.

use std::collections::HashMap;

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockAccess;

mod server;

use server::act;
use super::alter::blocks;
use super::grid::Grid;
use super::sim::perform;
use crate::build::{BuildJob, Builder, Structure, StatusHandle, Transform};
use crate::movement::Movement;
use crate::pathfind::process::{Act, Arbiter, BlockView, Decision, Look, ProcId, Target};
use crate::pathfind::{NavStatus, SearchJob, Step};
use crate::ActionError;

/// Ticks an act holds the bot, as live (house16 act timing, 2026-10-21): a placement aims for a tick, clicks
/// and waits out the tick after (~47 + 51 ms); a break adds ~100 ms of mining with the right tool; a restock
/// ~200 ms.
const PLACE_TICKS: u32 = 2;
const BREAK_TICKS: u32 = 4;
const RESTOCK_TICKS: u32 = 4;
/// Ticks until a block another bot changed shows in a bot's world (live echo ~146 ms).
pub const LIVE_DELAY: u32 = 3;
/// Ticks a background path search takes to answer; the sim's search itself is instant. Live one bot: ~310
/// ticks searching (18 s) over ~55 walks; 3 left the sim's moving at ~840 ticks against live's 1327.
const SEARCH_TICKS: u32 = 6;

struct Change {
    tick: u32,
    pos: BlockPos,
    /// The state before, as a registry id.
    old: u32,
    by: usize,
}

pub struct World<'a> {
    pub g: &'a Grid,
    pub inv: HashMap<String, u32>,
    pub stations: HashMap<BlockPos, HashMap<String, u32>>,
    /// Acts in order, for checking what came first.
    pub log: Vec<Act>,
    /// Feet where the last run ended.
    pub end: Vec3,
    /// Feet of the other simulated bots: blocks can't be placed into them.
    pub others: Vec<Vec3>,
    /// The bot acting now, and the tick.
    pub me: usize,
    pub now: u32,
    /// Ticks other bots' block changes stay hidden from a bot (0: everyone sees everything at once).
    pub delay: u32,
    changes: Vec<Change>,
    /// `now`, shared with journals (`Journal::clocked`).
    pub clock: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Background search answers held back until their tick.
    searches: Vec<(u32, tokio::sync::oneshot::Sender<crate::pathfind::Path>, crate::pathfind::Path)>,
    /// Sign text by cell and side (front `true`).
    pub signs: HashMap<(BlockPos, bool), String>,
    /// Cells a structure fills: `swarm_sim::run_many` times bots standing in them while still open.
    pub targets: std::collections::HashSet<BlockPos>,
}

impl<'a> World<'a> {
    pub fn new(g: &'a Grid, inv: &[(&str, u32)]) -> Self {
        let signs = HashMap::new();
        Self { g, inv: items(inv), stations: HashMap::new(), log: Vec::new(), end: [0.0; 3], others: Vec::new(), me: 0, now: 0, delay: 0, changes: Vec::new(), clock: Default::default(), searches: Vec::new(), signs, targets: Default::default() }
    }

    /// Sets a block as the server, remembering what it was for the bots that don't see it yet.
    pub(super) fn set(&mut self, pos: BlockPos, name: &str, props: &str) {
        if !name.ends_with("sign") {
            self.signs.retain(|(p, _), _| *p != pos);
        }
        if self.delay > 0 {
            let old = self.g.view.block(pos[0], pos[1], pos[2]);
            let horizon = self.now.saturating_sub(self.delay);
            self.changes.retain(|c| c.tick > horizon);
            self.changes.push(Change { tick: self.now, pos, old, by: self.me });
        }
        self.g.set_state(pos, name, props);
    }
}

pub fn items(inv: &[(&str, u32)]) -> HashMap<String, u32> {
    inv.iter().map(|(k, v)| (format!("minecraft:{k}"), *v)).collect()
}

struct SimLook<'a>(&'a World<'a>);

impl Look for SimLook<'_> {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        self.block(p).map(|b| b.solid())
    }

    fn find(&self, _: &Target) -> Option<Vec3> {
        None
    }

    fn block(&self, p: BlockPos) -> Option<BlockView> {
        let w = self.0;
        // The oldest change another bot made here that this bot can't see yet: what was there before it.
        let horizon = w.now.saturating_sub(w.delay);
        let hidden = w.changes.iter().find(|c| c.pos == p && c.by != w.me && c.tick > horizon).map(|c| c.old);
        let s = w.g.registry.get(hidden.unwrap_or_else(|| w.g.view.block(p[0], p[1], p[2])))?;
        Some(BlockView::of(s))
    }

    fn count(&self, item: &str) -> u32 {
        self.0.inv.get(item).copied().unwrap_or(0)
    }

    fn free_slots(&self) -> u32 {
        20
    }

    fn sign_text(&self, p: BlockPos, front: bool) -> Option<String> {
        self.0.signs.get(&(p, front)).cloned()
    }
}

pub fn view(g: &Grid, p: BlockPos) -> (String, String) {
    let s = g.registry.get(g.view.block(p[0], p[1], p[2])).expect("known");
    (s.name.to_string(), s.properties.to_string())
}

pub(super) fn take(w: &mut World, item: &str) -> Result<(), ActionError> {
    let n = w.inv.get_mut(item).filter(|n| **n > 0).ok_or_else(|| ActionError::NotPossible(format!("no {item}")))?;
    *n -= 1;
    Ok(())
}

/// `BUILD_WATCH=x,y,z`: every act touching that cell is printed (with the eye), for a cell's history.
fn watched(p: BlockPos) -> bool {
    static W: std::sync::OnceLock<Option<BlockPos>> = std::sync::OnceLock::new();
    let w = W.get_or_init(|| {
        let v: Vec<i32> = std::env::var("BUILD_WATCH").ok()?.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    });
    *w == Some(p)
}

/// A builder for `s` with its minimum corner at `origin`, run alone.
pub fn builder(s: &Structure, origin: BlockPos, stations: &[BlockPos]) -> (Builder, StatusHandle) {
    let mut job = BuildJob::new("sim.mcstructure", Transform { origin, ..Transform::default() });
    (job.stations, job.scaffold) = (stations.to_vec(), vec!["minecraft:dirt".into()]);
    let b = Builder::with_structure(job, s);
    let status = b.status();
    (b, status)
}

/// Runs `arb` from `start` until `id` finishes or `max_ticks` pass; returns the ticks taken (acts hold the bot
/// their live ticks, so ticks × 50 ms is comparable with a live run's wall time).
pub fn run(w: &mut World, start: Vec3, arb: &mut Arbiter, id: ProcId, max_ticks: u32) -> u32 {
    let mut m = Movement::new();
    m.start(start, 0.0, 0.0);
    let mut busy = 0;
    for tick in 0..max_ticks {
        w.now = tick;
        w.clock.store(u64::from(tick), std::sync::atomic::Ordering::Relaxed);
        if tick >= busy {
            busy = tick + step(w, &mut m, arb, tick);
        }
        if arb.outcome(id).is_some() {
            return tick;
        }
        m.tick(&w.g.physics());
    }
    max_ticks
}

/// One decision of `arb` and the world answering (movement not yet ticked); returns the ticks it holds the bot.
pub fn step(w: &mut World, m: &mut Movement, arb: &mut Arbiter, tick: u32) -> u32 {
    let world = w.g.physics();
    let pos = m.position().expect("started");
    let mut ticks = 1;
    let now = w.now;
    for (_, tx, path) in w.searches.extract_if(.., |s| s.0 <= now) {
        let _ = tx.send(path);
    }
    match arb.decide(pos, m.on_ground(), &SimLook(&*w)) {
        Decision::Go => {
            let pilot = arb.pilot_mut().expect("routed");
            let opts = pilot.search_opts(Some(blocks(64)), 20.0);
            let (terrain, mut held) = (w.g.terrain(), Vec::new());
            let result = pilot.drive(&terrain, m, opts, &mut |job: SearchJob| {
                let (tx, rx) = tokio::sync::oneshot::channel();
                held.push((now + SEARCH_TICKS, tx, job.run(&terrain)));
                rx
            });
            w.searches.extend(held);
            let mut placed = Vec::new();
            if result == Ok(NavStatus::Working) {
                let node = pilot.pending_work().expect("work due");
                placed = node.work.steps().filter_map(|s| if let Step::Place { against, face } = s { Some(face.adjacent(against)) } else { None }).collect();
                if node.work.steps().any(|s| matches!(s, Step::Place { against, face } if watched(face.adjacent(against))) || matches!(s, Step::Break(p) if watched(p))) {
                    eprintln!("WATCH tick {tick} path work {:?} from {pos:?}", node.work.steps().collect::<Vec<_>>());
                }
                perform(w.g, &world, m, &node);
                // Not held like acts: skipping decisions mid-pillar left the controls on and walks failed (sim
                // one bot 103 s -> 163 s with 3 failed walks). Live path work is ~11 s the sim leaves out.
                pilot.work_done();
            }
            if !matches!(result, Ok(NavStatus::Searching | NavStatus::Moving | NavStatus::Working)) {
                m.controls.stop();
            }
            if let Err(e) = &result {
                eprintln!("DRIVE FAIL tick {tick} at {pos:?}: {e}");
            }
            arb.placed(placed);
            arb.drove(result, Vec::new());
        }
        Decision::Act(pid, a) => {
            m.controls.stop();
            ticks = match &a {
                Act::Place { .. } | Act::Fill { .. } | Act::Use { .. } | Act::Sign { .. } | Act::UseOn { .. } => PLACE_TICKS,
                Act::Break { .. } => BREAK_TICKS,
                Act::Restock { .. } => RESTOCK_TICKS,
            };
            let result = act(w, m, &a);
            if let Err(e) = &result {
                eprintln!("ACT FAIL tick {tick} {a:?}: {e}");
            }
            arb.acted(pid, &a, &result);
        }
        Decision::Hold | Decision::Idle | Decision::Empty => m.controls.stop(),
    }
    w.end = pos;
    ticks
}

/// Why each open placement of `s` is not being done from where the last run ended.
pub fn explain(w: &World, s: &Structure, origin: BlockPos) -> Vec<String> {
    builder(s, origin, &[]).0.explain(&SimLook(w), w.end)
}
