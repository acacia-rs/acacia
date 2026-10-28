//! The builder in the headless simulation: the grid stands in for the server (placed states from
//! `place_model`), acts take the ticks they take live, and with `delay` a bot sees other bots' blocks late.

use std::collections::HashMap;

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockAccess;

use super::alter::blocks;
use super::grid::Grid;
use super::place_model::bds_state;
use super::sim::perform;
use crate::build::{BuildJob, Builder, Structure, StatusHandle, Transform};
use crate::interact::{block_distance, Face, BLOCK_REACH};
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
}

impl<'a> World<'a> {
    pub fn new(g: &'a Grid, inv: &[(&str, u32)]) -> Self {
        Self { g, inv: items(inv), stations: HashMap::new(), log: Vec::new(), end: [0.0; 3], others: Vec::new(), me: 0, now: 0, delay: 0, changes: Vec::new(), clock: Default::default(), searches: Vec::new() }
    }

    /// Sets a block as the server, remembering what it was for the bots that don't see it yet.
    fn set(&mut self, pos: BlockPos, name: &str, props: &str) {
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
}

pub fn view(g: &Grid, p: BlockPos) -> (String, String) {
    let s = g.registry.get(g.view.block(p[0], p[1], p[2])).expect("known");
    (s.name.to_string(), s.properties.to_string())
}

fn eye(m: &Movement) -> Vec3 {
    let f = m.position().expect("started");
    [f[0], f[1] + 1.62, f[2]]
}

fn in_body(m: &Movement, p: BlockPos) -> bool {
    body_at(m.position().expect("started"), p)
}

fn body_at(f: Vec3, p: BlockPos) -> bool {
    let o = |lo: f32, hi: f32, c: i32| lo < (c + 1) as f32 && hi > c as f32;
    o(f[0] - 0.3, f[0] + 0.3, p[0]) && o(f[1], f[1] + 1.8, p[1]) && o(f[2] - 0.3, f[2] + 0.3, p[2])
}

fn take(w: &mut World, item: &str) -> Result<(), ActionError> {
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

/// Does an act as the server would, from where the simulated player stands.
fn act(w: &mut World, m: &Movement, act: &Act) -> Result<(), ActionError> {
    let r = act_inner(w, m, act);
    let cell = match act {
        Act::Place { against, face, .. } => face.adjacent(*against),
        Act::Break { pos } | Act::Fill { pos, .. } => *pos,
        Act::Restock { station, .. } => *station,
        Act::UseOn { cell, .. } => *cell,
    };
    if watched(cell) || matches!(act, Act::Place { against, .. } if watched(*against)) {
        eprintln!("WATCH bot {} tick {} eye {:?} {act:?} -> {r:?}; now {:?}", w.me, w.now, eye(m), view(w.g, cell));
    }
    r
}

fn act_inner(w: &mut World, m: &Movement, act: &Act) -> Result<(), ActionError> {
    w.log.push(act.clone());
    let e = eye(m);
    let far = |p: BlockPos| block_distance(e, p) > BLOCK_REACH;
    let blocked = |w: &World, p: BlockPos| in_body(m, p) || w.others.iter().any(|o| body_at(*o, p));
    // The server judges by the true world, not what this bot sees.
    let solid = |w: &World, p: BlockPos| w.g.registry.get(w.g.view.block(p[0], p[1], p[2])).map(BlockView::of);
    match act {
        Act::Place { item, against, face, click } => {
            let pos = face.adjacent(*against);
            let cell = solid(w, pos).expect("loaded");
            if far(*against) || solid(w, *against).is_none_or(|b| !b.solid()) || blocked(w, pos) || (cell.solid() && !cell.name.ends_with("_slab")) {
                return Err(ActionError::Rejected(format!("place {item} at {pos:?} from {e:?}")));
            }
            if let Some(state) = super::redstone_sim::place(w.g, item, *face, pos) {
                let (name, props) = state.ok_or_else(|| ActionError::Rejected(format!("{item} refused at {pos:?}")))?;
                take(w, item)?;
                w.set(pos, &name, &props);
                super::redstone_sim::join(w.g, pos);
                return Ok(());
            }
            let point = [against[0] as f32 + click[0], against[1] as f32 + click[1], against[2] as f32 + click[2]];
            let (name, props) = bds_state(item, *face, *click, e, point, cell.name, pos).ok_or_else(|| ActionError::Rejected("refused".into()))?;
            take(w, item)?;
            w.set(pos, &name, &props);
            Ok(())
        }
        Act::Break { pos } => {
            if far(*pos) {
                return Err(ActionError::NotPossible(format!("break {pos:?} from {e:?}")));
            }
            w.set(*pos, "air", "");
            Ok(())
        }
        Act::Fill { pos, blocks } => {
            let reach = Face::ALL.iter().any(|f| {
                let o = f.offset();
                let a = [pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]];
                solid(w, a).is_some_and(|b| b.solid()) && !far(a)
            });
            let item = blocks.iter().find(|b| w.inv.get(*b).copied().unwrap_or(0) > 0).cloned();
            match item {
                Some(item) if reach && !blocked(w, *pos) => {
                    take(w, &item)?;
                    w.set(*pos, item.trim_start_matches("minecraft:"), "");
                    Ok(())
                }
                _ => Err(ActionError::NotPossible(format!("fill {pos:?} from {e:?}"))),
            }
        }
        Act::UseOn { item, against, face, .. } if !far(*against) => super::redstone_sim::use_on(w.g, &mut w.inv, item, *against, *face),
        Act::UseOn { .. } => Err(ActionError::NotPossible("use out of reach".into())),
        Act::Restock { station, items } => {
            if far(*station) {
                return Err(ActionError::NotPossible("station out of reach".into()));
            }
            let stock = w.stations.get_mut(station).ok_or_else(|| ActionError::NotPossible("no station".into()))?;
            for (item, want) in items {
                let have = stock.get(item).copied().unwrap_or(0);
                let moved = have.min(want.div_ceil(64) * 64);
                stock.insert(item.clone(), have - moved);
                *w.inv.entry(item.clone()).or_default() += moved;
            }
            Ok(())
        }
    }
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
                Act::Place { .. } | Act::Fill { .. } | Act::UseOn { .. } => PLACE_TICKS,
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
