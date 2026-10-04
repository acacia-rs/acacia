//! Async execution: plan, follow, re-plan on trouble (azalea `execute/`).

use std::sync::Arc;
use std::time::Duration;

use acacia_physics::{BlockPos, Vec3};
use acacia_world::{BlockRegistry, ChunkView};
use tokio::task::JoinHandle;

use super::astar::{search, Path, SearchOpts};
use super::follow::{FollowStatus, Follower, Sense};
use super::execute;
use super::goal::Goal;
use super::kit::{DEFAULT_SCAFFOLD, Kit};
use super::snapshot::WorldBlocks;
use super::terrain::{Blocks, Terrain};
use crate::{ActionError, Bot};

/// Nodes ahead re-checked every tick for world changes.
const LOOKAHEAD: usize = 6;
/// A search that finishes within this runs inline, before the next tick (azalea's instant path).
const QUICK_NODES: usize = 5_000;
const QUICK_BUDGET: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
pub struct GotoOpts {
    /// Search settings; its `kit` is refilled from the inventory before every search.
    pub search: SearchOpts,
    /// Re-plans after getting stuck, knocked off the path, finding no path or failing a dig, place
    /// or door before giving up. Continuing from the end of a partial path and re-planning after
    /// a teleport or a block change are free.
    pub max_replans: u32,
    /// How close to the final node's centre (horizontally, blocks) counts as arrived.
    pub tolerance: f32,
    /// Item identifiers of the throwaway blocks bridging and pillaring may use, in preference order.
    pub scaffold: Vec<String>,
}

impl Default for GotoOpts {
    fn default() -> Self {
        let scaffold = DEFAULT_SCAFFOLD.iter().map(|s| s.to_string()).collect();
        Self { search: SearchOpts::default(), max_replans: 10, tolerance: 0.25, scaffold }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavStatus {
    Searching,
    Moving,
    /// Digging, placing or opening a door for the next move.
    Working,
    Arrived,
}

/// Step-wise [`Bot::goto`]: call [`Navigator::tick`] until it returns `Arrived`, reading progress
/// in between. Dropping it mid-way leaves the controls set; call [`Bot::stop_pathing`].
pub struct Navigator {
    goal: Goal,
    opts: GotoOpts,
    follower: Option<Follower>,
    partial: bool,
    pending: Option<JoinHandle<Path>>,
    replans: u32,
    teleports: Option<u32>,
}

impl Navigator {
    pub fn new(goal: Goal, opts: GotoOpts) -> Self {
        Self { goal, opts, follower: None, partial: false, pending: None, replans: 0, teleports: None }
    }

    pub fn goal(&self) -> &Goal {
        &self.goal
    }

    /// Re-plans counted against [`GotoOpts::max_replans`].
    pub fn replans(&self) -> u32 {
        self.replans
    }

    /// Path nodes still ahead (0 while searching).
    pub fn remaining(&self) -> usize {
        self.follower.as_ref().map_or(0, Follower::remaining)
    }

    /// The current path leads towards the goal but not to it.
    pub fn is_partial(&self) -> bool {
        self.partial
    }

    pub fn follower(&self) -> Option<&Follower> {
        self.follower.as_ref()
    }

    /// Sets the controls for one tick and lets it pass (except on arrival). Controls are stopped
    /// on arrival and on error.
    pub async fn tick(&mut self, bot: &mut Bot) -> Result<NavStatus, ActionError> {
        let status = match self.poll(bot).await {
            Ok(s) => s,
            Err(e) => {
                bot.stop_pathing();
                return Err(e);
            }
        };
        if status == NavStatus::Arrived {
            bot.stop_pathing();
        } else {
            bot.next_tick(|_, _| false).await?;
        }
        Ok(status)
    }

    async fn poll(&mut self, bot: &mut Bot) -> Result<NavStatus, ActionError> {
        if bot.movement.is_none() {
            return Err(ActionError::NotPossible("pathfinding needs a physics bot (BotConfig.physics)".into()));
        }
        if let Some(handle) = &mut self.pending {
            if !handle.is_finished() {
                return Ok(NavStatus::Searching);
            }
            let path = handle.await.map_err(|e| ActionError::NotPossible(format!("path search failed: {e}")))?;
            self.pending = None;
            let pos = bot.movement.as_ref().and_then(|m| m.position()).unwrap_or_default();
            self.accept(path, pos)?;
        }
        let status = self.drive(bot)?;
        if let Some(node) = self.follower.as_ref().and_then(Follower::pending_work).copied()
            && status == NavStatus::Working
        {
            match execute::perform(bot, &node, &self.opts.scaffold).await {
                Ok(()) => self.follower.as_mut().into_iter().for_each(Follower::work_done),
                Err(ActionError::Disconnected) => return Err(ActionError::Disconnected),
                Err(e) => {
                    tracing::debug!(%e, ?node, "path work failed; re-planning");
                    self.follower = None;
                    self.count_replan(e.to_string())?;
                }
            }
        }
        Ok(status)
    }

    fn drive(&mut self, bot: &mut Bot) -> Result<NavStatus, ActionError> {
        let s = &self.opts.search;
        let kit = (s.allow_dig || s.allow_bridge).then(|| bot.path_kit(&self.opts.scaffold));
        let (Some(world), Some(movement)) = (bot.world.as_ref(), bot.movement.as_mut()) else {
            return Ok(NavStatus::Searching);
        };
        let (Some(view), Some(registry), Some(pos)) = (world.view(), world.registry(), movement.position()) else {
            return Ok(NavStatus::Searching);
        };
        let terrain = Terrain::new(view, registry.as_ref());
        if self.teleports.replace(movement.teleports).is_some_and(|t| t != movement.teleports) {
            self.follower = None;
        }
        if self.follower.as_ref().is_some_and(|f| !still_valid(&terrain, f)) {
            tracing::debug!("path blocked by a world change; re-planning");
            self.follower = None;
        }
        let Some(follower) = &mut self.follower else {
            movement.controls.stop();
            self.plan(&terrain, view, registry, pos, kit)?;
            return Ok(NavStatus::Searching);
        };
        let feet = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
        let sense = Sense { pos, on_ground: movement.on_ground(), in_water: terrain.cell(feet).is_water() };
        match follower.tick(&sense, &mut movement.controls) {
            FollowStatus::Moving => {}
            FollowStatus::Work => return Ok(NavStatus::Working),
            FollowStatus::Arrived if !self.partial => return Ok(NavStatus::Arrived),
            FollowStatus::Arrived => self.follower = None,
            status @ (FollowStatus::Stuck | FollowStatus::OffPath) => {
                tracing::debug!(?status, ?pos, "re-planning");
                self.count_replan(format!("{status:?} {} times", self.replans))?;
                self.follower = None;
            }
        }
        Ok(NavStatus::Moving)
    }

    fn plan(&mut self, terrain: &Terrain<&ChunkView>, view: &ChunkView, registry: &Arc<BlockRegistry>, pos: Vec3, kit: Option<Kit>) -> Result<(), ActionError> {
        let mut opts = self.opts.search.clone();
        if let Some(kit) = kit {
            opts.kit = kit;
        }
        let quick = search(terrain, pos, &self.goal, &SearchOpts { max_nodes: QUICK_NODES, budget: QUICK_BUDGET, ..opts.clone() });
        if quick.complete {
            return self.accept(quick, pos);
        }
        let (world, registry, goal) = (view.world().clone(), registry.clone(), self.goal.clone());
        self.pending = Some(tokio::task::spawn_blocking(move || {
            search(&Terrain::new(WorldBlocks::new(world), &registry), pos, &goal, &opts)
        }));
        Ok(())
    }

    fn accept(&mut self, path: Path, pos: Vec3) -> Result<(), ActionError> {
        tracing::debug!(nodes = path.nodes.len(), complete = path.complete, cost = path.cost, visited = path.visited, "path found");
        if path.nodes.is_empty() && !path.complete {
            self.count_replan("no path".into())?;
        }
        self.partial = !path.complete;
        self.follower = Some(Follower::new(path.nodes, pos, self.opts.tolerance));
        Ok(())
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

impl Bot {
    /// Walks to `goal` (physics bots only). See [`Navigator`] for step-wise control.
    pub async fn goto(&mut self, goal: Goal) -> Result<(), ActionError> {
        self.goto_with(goal, GotoOpts::default()).await
    }

    pub async fn goto_with(&mut self, goal: Goal, opts: GotoOpts) -> Result<(), ActionError> {
        let mut nav = Navigator::new(goal, opts);
        while nav.tick(self).await? != NavStatus::Arrived {}
        Ok(())
    }

    /// Releases the movement controls, e.g. after dropping a `goto` future or a [`Navigator`].
    pub fn stop_pathing(&mut self) {
        if let Some(c) = self.controls() {
            c.stop();
        }
    }

    /// Feet block position of a physics bot (for building goals).
    pub fn block_position(&self) -> Option<BlockPos> {
        let p = self.movement.as_ref()?.position()?;
        Some([p[0].floor() as i32, (p[1] + 0.01).floor() as i32, p[2].floor() as i32])
    }
}
