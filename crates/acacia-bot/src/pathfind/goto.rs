//! Async execution: plan, follow, re-plan on trouble (azalea `execute/`). The logic is [`Pilot`]'s; this adds
//! the server side: world changes done by the bot's actions, and waiting for the next tick.

use std::sync::Arc;

use acacia_physics::BlockPos;
use acacia_world::{BlockRegistry, ChunkView};

use super::execute;
use super::goal::Goal;
use super::hazard::{self, Hazards};
use super::work::Step;
use super::kit::DEFAULT_SCAFFOLD;
use super::pilot::Pilot;
use super::terrain::Terrain;
use super::{Follower, SearchOpts};
use crate::movement::Movement;
use crate::state::GameState;
use crate::{ActionError, Bot};

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
    /// Run straight at nodes further on over flat ground instead of the path's 8-direction staircase.
    pub shortcuts: bool,
    /// Seeds the followers' aim jitter (`Human`); random when `None`.
    pub seed: Option<u64>,
    /// Give up after this many ticks without coming a block closer to the goal; `None` never.
    pub give_up_ticks: Option<u32>,
}

impl Default for GotoOpts {
    fn default() -> Self {
        let scaffold = DEFAULT_SCAFFOLD.iter().map(|s| s.to_string()).collect();
        Self { search: SearchOpts::default(), max_replans: 10, tolerance: 0.25, scaffold, shortcuts: true, seed: None, give_up_ticks: Some(600) }
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
    pilot: Pilot,
}

impl Navigator {
    pub fn new(goal: Goal, opts: GotoOpts) -> Self {
        Self { pilot: Pilot::new(goal, opts) }
    }

    /// Macro moves that diverged live (each re-planned without stopping).
    pub fn divergences(&self) -> u32 {
        self.pilot.divergences()
    }

    pub fn goal(&self) -> &Goal {
        self.pilot.goal()
    }

    /// Re-plans counted against [`GotoOpts::max_replans`].
    pub fn replans(&self) -> u32 {
        self.pilot.replans()
    }

    /// Path nodes still ahead (0 while searching).
    pub fn remaining(&self) -> usize {
        self.pilot.follower().map_or(0, Follower::remaining)
    }

    /// The current path leads towards the goal but not to it.
    pub fn is_partial(&self) -> bool {
        self.pilot.is_partial()
    }

    pub fn follower(&self) -> Option<&Follower> {
        self.pilot.follower()
    }

    /// The navigator's core, for its counters (searches, segment joins).
    pub fn pilot(&self) -> &Pilot {
        &self.pilot
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
        step(&mut self.pilot, bot).await.0
    }
}

/// A block a path's work broke, with its identifier.
pub(super) type Dug = (BlockPos, String);

/// What a path's work changed this tick, failed or not.
#[derive(Default)]
pub(super) struct Worked {
    pub dug: Vec<Dug>,
    /// Cells it placed a block into.
    pub placed: Vec<BlockPos>,
}

/// The pilot's field this tick from the entities around the bot ([`Pilot::perceive`]).
fn perceive(pilot: &mut Pilot, state: &GameState, movement: &Movement, view: &ChunkView, registry: &BlockRegistry) -> Option<Arc<Hazards>> {
    let st = movement.state()?;
    let threats = hazard::Threats::near(&state.entities, st.pos, hazard::FIELD_RADIUS);
    let blocks = Terrain::new(view, registry);
    let sees = |a, b| hazard::clear_line(&blocks, a, b);
    let eye = [st.pos[0], st.pos[1] + acacia_physics::constants::DEFAULT_PLAYER_HEIGHT_OFFSET, st.pos[2]];
    pilot.perceive(threats, &hazard::View { eye, yaw: st.yaw, health: state.player.health, sees: &sees })
}

/// [`perceive`] for a pilot not driven this tick (one that holds at its goal still notices a threat leave).
pub(super) fn sense(pilot: &mut Pilot, bot: &Bot) {
    let (Some(world), Some(movement)) = (bot.world.as_ref(), bot.movement.as_ref()) else { return };
    if let (Some(view), Some(registry)) = (world.view(), world.registry()) {
        perceive(pilot, &bot.state, movement, view, registry.as_ref());
    }
}

/// Drives `pilot` for one tick (without letting it pass) and does any work due; also returns what the work changed.
pub(super) async fn step(pilot: &mut Pilot, bot: &mut Bot) -> (Result<NavStatus, ActionError>, Worked) {
    if bot.movement.is_none() {
        return (Err(ActionError::NotPossible("pathfinding needs a physics bot (BotConfig.physics)".into())), Worked::default());
    }
    let s = &pilot.opts().search;
    let kit = (s.allow_dig || s.allow_bridge).then(|| bot.path_kit(&pilot.opts().scaffold));
    let mut opts = pilot.search_opts(kit, bot.state.player.hunger);
    let (Some(world), Some(movement)) = (bot.world.as_ref(), bot.movement.as_mut()) else {
        return (Ok(NavStatus::Searching), Worked::default());
    };
    let (Some(view), Some(registry)) = (world.view(), world.registry()) else {
        return (Ok(NavStatus::Searching), Worked::default());
    };
    opts.hazards = perceive(pilot, &bot.state, movement, view, registry.as_ref());
    let terrain = Terrain::new(view, registry.as_ref()).with_hazards(opts.hazards.clone());
    let (snapshot, reg) = (view.world().clone(), registry.clone());
    let status = match pilot.drive(&terrain, movement, opts, &mut |job| job.spawn_blocking(snapshot.clone(), reg.clone())) {
        Ok(s) => s,
        Err(e) => return (Err(e), Worked::default()),
    };
    let Some(node) = pilot.pending_work().filter(|_| status == NavStatus::Working) else { return (Ok(status), Worked::default()) };
    let solid = |bot: &Bot, p: BlockPos| bot.block_at(p).is_some_and(|(_, s)| !s.is_air() && !s.is_liquid());
    let name = |bot: &Bot, p: BlockPos| bot.block_at(p).filter(|(_, s)| !s.is_air() && !s.is_liquid()).map(|(_, s)| s.name.to_string());
    let breaks: Vec<_> = node.work.steps().filter_map(|s| if let Step::Break(p) = s { Some(p) } else { None }).collect();
    let places: Vec<_> = node.work.steps().filter_map(|s| if let Step::Place { against, face } = s { Some(face.adjacent(against)) } else { None }).collect();
    let before: Vec<_> = breaks.into_iter().filter_map(|p| Some((p, name(bot, p)?))).collect();
    let empty: Vec<_> = places.into_iter().filter(|p| !solid(bot, *p)).collect();
    let done = execute::perform(bot, &node, &pilot.opts().scaffold).await;
    let dug = before.into_iter().filter(|(p, _)| !solid(bot, *p)).collect();
    let worked = Worked { dug, placed: empty.into_iter().filter(|p| solid(bot, *p)).collect() };
    let status = match done {
        Ok(()) => Ok(status).inspect(|_| pilot.work_done()),
        Err(ActionError::Disconnected) => Err(ActionError::Disconnected),
        Err(e) => {
            tracing::debug!(%e, ?node, "path work failed; re-planning");
            pilot.work_failed(e.to_string()).map(|()| status)
        }
    };
    (status, worked)
}
