//! The navigator's core, network-free: plans, follows, plans the next segment ahead and splices it in, and
//! re-plans on trouble. Driven once per tick with the movement and the terrain: by
//! [`Navigator`](super::Navigator) on a server, by the simulation in tests. Design: docs/pathfind-plan.md "C1".

mod access;
mod ahead;
mod alarm;
mod aware;
mod guard;
mod watch;

use std::sync::Arc;
use std::time::Duration;

use acacia_physics::{PlayerState, Vec3};
use tokio::sync::oneshot::error::TryRecvError;

use super::astar::{Path, SearchOpts, search_cached};
use super::cache::RolloutCache;
use super::favour::Favour;
use super::follow::{FollowStatus, Follower, Sense};
use super::goal::Goal;
use super::goto::{GotoOpts, NavStatus};
use super::hazard::{Hazards, Senses, Striker};
use super::job::{Answer, SearchJob, Spawn};
use super::kit::Kit;
use super::profile::MovementProfile;
use super::terrain::{Blocks, Terrain};
use super::PathNode;
use crate::cadence::{seed, splitmix};
use crate::movement::Movement;
use crate::ActionError;
use ahead::Ahead;
use alarm::Alarm;
use guard::Guard;
use watch::Watch;

/// Nodes ahead re-checked every tick for world changes.
const LOOKAHEAD: usize = 6;
/// A search that finishes within this runs inline, before the next tick (azalea's instant path).
const QUICK_NODES: usize = 5_000;
const QUICK_BUDGET: Duration = Duration::from_millis(10);
/// Macro rollout ticks the inline search may simulate (~5 µs each in release).
const QUICK_ROLLOUT_TICKS: u64 = 1_000;
/// Divergences in a row that re-plan for free; each one past it counts against `max_replans`.
const MAX_DIVERGED: u32 = 5;
const HOP_MIN_FOOD: f32 = 6.0;
/// Back-offs (20, 40, 80 ticks) waited for a solid entity to clear the way before an unreachable goal fails.
const MAX_BLOCKED_WAITS: u32 = 3;
/// `Human` waits up to this many ticks longer each time, so blocked bots don't all move off together.
const HUMAN_BLOCKED_JITTER: u64 = 15;
/// Horizontal speed (blocks/tick) above which a re-plan takes a quick partial path rather than stop.
const MOVING: f32 = 0.1;

pub struct Pilot {
    goal: Goal,
    opts: GotoOpts,
    follower: Option<Follower>,
    partial: bool,
    /// The search for a new path from the live state, while it runs.
    pending: Option<Answer>,
    ahead: Ahead,
    /// Bumped whenever the follower is replaced: a plan-ahead for an older one is dropped.
    generation: u32,
    replans: u32,
    teleports: Option<u32>,
    /// Macro moves diverged in a row; free re-plans up to `MAX_DIVERGED`.
    diverged: u32,
    diverged_in_air: bool,
    divergences: u32,
    /// The dropped path's cells, favoured by re-plans until one is accepted.
    favour: Option<Arc<Favour>>,
    guard: Guard,
    /// Macro-move results kept across re-plans, shared with the searches running.
    cache: Arc<RolloutCache>,
    /// Seeds each follower's aim jitter.
    rng: u64,
    watch: Watch,
    alarm: Alarm,
    senses: Senses,
    striker: Striker,
    field: Option<Arc<Hazards>>,
    perceived_at: Option<u64>,
    /// Searches in a row a solid entity may have closed off, waited out.
    blocked_waits: u32,
}

impl Pilot {
    pub fn new(goal: Goal, opts: GotoOpts) -> Self {
        Self::with_cache(goal, opts, Arc::default())
    }

    /// A pilot reusing another's macro-move results (the cache keys them by profile and player).
    pub fn with_cache(goal: Goal, opts: GotoOpts, cache: Arc<RolloutCache>) -> Self {
        let (rng, guard) = (opts.seed.unwrap_or_else(seed), Guard::new(opts.give_up_ticks));
        let striker = Striker::new(opts.search.profile, rng.rotate_left(29));
        let (follower, pending, ahead, favour) = (None, None, Ahead::default(), None);
        Self { goal, opts, follower, partial: false, pending, ahead, generation: 0, replans: 0, teleports: None, diverged: 0, diverged_in_air: false, divergences: 0, favour, guard, cache, rng, watch: Watch::default(), alarm: Alarm::default(), senses: Senses::new(rng.rotate_left(17)), striker, field: None, perceived_at: None, blocked_waits: 0 }
    }

    /// Something else had the controls for a while: drop the path (re-planned from here, favoured) and any
    /// search started from where the bot was. Not counted as a re-plan.
    pub fn interrupt(&mut self) {
        self.drop_follower(true);
        (self.pending, self.diverged_in_air) = (None, false);
    }

    /// The search settings for now: the inventory's kit, and no flat hops at food 6 or less (sprinting
    /// stops there, and hops burn the most food).
    pub fn search_opts(&self, kit: Option<Kit>, food: f32) -> SearchOpts {
        let mut opts = self.opts.search.clone();
        if let Some(kit) = kit {
            opts.kit = kit;
        }
        opts.hops &= food > HOP_MIN_FOOD;
        opts
    }

    /// The node whose work is due, while [`Pilot::drive`] returns `Working`.
    pub fn pending_work(&self) -> Option<PathNode> {
        self.follower.as_ref().and_then(Follower::pending_work).copied()
    }

    pub fn work_done(&mut self) {
        self.follower.as_mut().into_iter().for_each(Follower::work_done);
    }

    /// The work failed: re-plan (counted).
    pub fn work_failed(&mut self, why: String) -> Result<(), ActionError> {
        self.drop_follower(false);
        self.count_replan(why)
    }

    /// Sets `movement`'s controls for the next tick. `spawn` starts the searches that do not fit in the tick.
    pub fn drive<B: Blocks>(&mut self, terrain: &Terrain<B>, movement: &mut Movement, opts: SearchOpts, spawn: Spawn) -> Result<NavStatus, ActionError> {
        let Some(live) = movement.state().cloned() else { return Ok(NavStatus::Searching) };
        self.guard.tick(&self.goal, live.pos)?;
        if self.hold_after_setback(movement) {
            return Ok(NavStatus::Searching);
        }
        if let Some(answer) = &mut self.pending {
            let path = match answer.try_recv() {
                Err(TryRecvError::Empty) => return Ok(NavStatus::Searching),
                Err(TryRecvError::Closed) => return Err(ActionError::NotPossible("path search failed".into())),
                Ok(path) => path,
            };
            self.pending = None;
            self.accept(path, live.pos)?;
        }
        if self.teleports.replace(movement.teleports).is_some_and(|t| t != movement.teleports) {
            self.drop_follower(false);
        }
        if self.follower.as_ref().is_some_and(|f| !still_valid(terrain, f)) {
            tracing::debug!("path blocked by a world change; re-planning");
            self.drop_follower(true);
        }
        if self.follower.as_ref().is_some_and(|f| self.alarm.tick(opts.hazards.as_deref(), f, live.pos)) {
            tracing::debug!(?live.pos, "an entity moved onto the path ahead; re-planning");
            self.drop_follower(false);
        }
        self.poll_ahead();
        if self.follower.is_none() {
            // After a divergence the controls carry the bot on through the air while it re-plans.
            if !self.diverged_in_air || movement.on_ground() {
                movement.controls.stop();
            }
            if !self.guard.waiting() {
                self.plan(terrain, live.clone(), opts.clone(), true, &mut *spawn)?;
            }
        }
        // A quick search that found its path drives in the same tick (live builder: a standing tick per walk and
        // per act mid-walk was a quarter of its moving time).
        let Some(follower) = &mut self.follower else { return Ok(NavStatus::Searching) };
        follower.set_hazards(opts.hazards.clone());
        let pos = live.pos;
        let feet = pos.map(|v| v.floor() as i32);
        let sense = Sense { pos, on_ground: movement.on_ground(), in_water: terrain.cell(feet).is_water() };
        let fork = movement.fork();
        let status = follower.tick(&sense, fork.as_ref(), terrain, &mut movement.controls);
        let (index, short) = (follower.index(), follower.spend().short);
        match status {
            FollowStatus::Moving => {
                if index > 0 {
                    self.diverged = 0;
                }
                self.plan_ahead(&live, &opts, spawn);
            }
            FollowStatus::Diverged => {
                // Out of rollout budget this tick: no inline search on top.
                tracing::debug!(?pos, inline = !short, "macro move diverged; re-planning in place");
                self.diverged += 1;
                self.divergences += 1;
                if self.diverged > MAX_DIVERGED {
                    self.count_replan(format!("diverged {} times in a row", self.diverged))?;
                    self.diverged = 0;
                }
                self.drop_follower(true);
                self.diverged_in_air = !movement.on_ground();
                self.plan(terrain, live, opts, !short, spawn)?;
            }
            FollowStatus::Work => {
                if let Some(node) = self.pending_work() {
                    self.watch.work(&node)?;
                }
                return Ok(NavStatus::Working);
            }
            FollowStatus::Arrived if !self.partial => return Ok(NavStatus::Arrived),
            // At the end of a segment before the next one is planned: it starts here, so wait for it.
            FollowStatus::Arrived if self.ahead.running() => return Ok(NavStatus::Searching),
            FollowStatus::Arrived => self.drop_follower(false),
            status @ (FollowStatus::Stuck | FollowStatus::OffPath) => {
                tracing::debug!(?status, ?pos, "re-planning");
                self.count_replan(format!("{status:?} {} times", self.replans))?;
                self.drop_follower(true);
            }
        }
        Ok(NavStatus::Moving)
    }

    /// Searches from the live state, so the plan starts with the momentum the bot has (a re-plan after a
    /// divergence keeps running). `inline`: try a quick search before the tick ends, else only in the background.
    fn plan<B: Blocks>(&mut self, terrain: &Terrain<B>, live: PlayerState, mut opts: SearchOpts, inline: bool, spawn: Spawn) -> Result<(), ActionError> {
        opts.favour = self.favour.clone();
        self.guard.searched(live.on_ground);
        if inline {
            let quick_opts = SearchOpts { max_nodes: QUICK_NODES, budget: QUICK_BUDGET, max_rollout_ticks: QUICK_ROLLOUT_TICKS, ..opts.clone() };
            let quick = search_cached(terrain, &live, &self.goal, &quick_opts, &self.cache);
            // A plan whose macro budget ran out may have missed faster macro moves: search again in full. Moving, a
            // partial one beats stopping for the full search: plan-ahead extends it from its end.
            let moving = live.vel[0].hypot(live.vel[2]) > MOVING;
            if quick.unreachable || (quick.complete && !quick.macro_stats.exhausted) || (moving && !quick.complete && !quick.nodes.is_empty()) {
                return self.accept(quick, live.pos);
            }
        }
        let job = SearchJob { from: live, goal: self.goal.clone(), opts, cache: self.cache.clone() };
        self.pending = Some(spawn(job));
        Ok(())
    }

    fn accept(&mut self, path: Path, pos: Vec3) -> Result<(), ActionError> {
        tracing::debug!(nodes = path.nodes.len(), complete = path.complete, unreachable = path.unreachable, cost = path.cost, visited = path.visited, "path found");
        // A solid entity may be what closes the way: wait for it to move before giving up (design decision 5).
        if path.unreachable && self.field.as_ref().is_some_and(|h| !h.solids.is_empty()) && self.blocked_waits < MAX_BLOCKED_WAITS {
            self.blocked_waits += 1;
            let extra = if self.opts.search.profile == MovementProfile::Human { (splitmix(&mut self.rng) % HUMAN_BLOCKED_JITTER) as u32 } else { 0 };
            self.guard.wait_out(extra);
            return Ok(());
        }
        if path.unreachable {
            return Err(ActionError::NotPossible(format!("cannot reach {:?} from {pos:?}: every reachable cell searched", self.goal)));
        }
        (self.favour, self.diverged_in_air) = (None, false);
        if path.nodes.is_empty() && !path.complete {
            // No way on from here (yet): wait before searching again rather than re-search every tick.
            self.count_replan("no path".into())?;
            self.guard.back_off();
            return Ok(());
        }
        self.guard.found();
        self.alarm.accepted();
        self.blocked_waits = 0;
        self.partial = !path.complete;
        self.generation += 1;
        self.ahead.clear();
        let mut follower = Follower::new(path.nodes, pos, self.opts.tolerance).moving(self.opts.search.profile, splitmix(&mut self.rng));
        follower.set_ends_at_goal(path.complete);
        self.follower = Some(follower.shortcuts(self.opts.shortcuts));
        Ok(())
    }

    /// Drops the path followed now; `favour`: the re-plan keeps to its cells where it can.
    fn drop_follower(&mut self, favour: bool) {
        if let Some(f) = self.follower.take().filter(|_| favour) {
            self.favour = Some(Arc::new(Favour::new(f.ahead().iter().map(|n| n.pos))));
        }
        self.ahead.clear();
    }

    fn count_replan(&mut self, why: String) -> Result<(), ActionError> {
        self.replans += 1;
        if self.replans > self.opts.max_replans {
            return Err(ActionError::NotPossible(format!("cannot reach {:?}: {why}", self.goal)));
        }
        Ok(())
    }
}

/// The next nodes still offer the same footing (up to the first that waits for work: its footing
/// comes from that work).
fn still_valid<B: Blocks>(terrain: &Terrain<B>, f: &Follower) -> bool {
    f.standing_ahead().iter().take(LOOKAHEAD).all(|n| terrain.spot(n.pos).is_some_and(|s| (s.feet - n.feet).abs() < 0.01))
}
