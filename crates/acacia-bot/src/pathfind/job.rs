//! Searches off the tick: the pilot hands a [`SearchJob`] to a spawner and polls its [`Answer`] once per tick.

use std::sync::Arc;

use acacia_physics::PlayerState;
use acacia_world::{BlockRegistry, World};
use tokio::sync::oneshot;

use super::astar::{Path, SearchOpts, search_cached};
use super::cache::RolloutCache;
use super::goal::Goal;
use super::snapshot::WorldBlocks;
use super::terrain::{Blocks, Terrain};

/// A search's result, once it is done.
pub type Answer = oneshot::Receiver<Path>;

/// Starts a search somewhere off the tick.
pub type Spawn<'a> = &'a mut dyn FnMut(SearchJob) -> Answer;

pub struct SearchJob {
    /// The state searched from (live, or synthesized at a path's end).
    pub from: PlayerState,
    pub goal: Goal,
    pub opts: SearchOpts,
    /// Macro-move results shared by every search of one goto, running ones included.
    pub cache: Arc<RolloutCache>,
}

impl SearchJob {
    pub fn run<B: Blocks>(&self, t: &Terrain<B>) -> Path {
        search_cached(t, &self.from, &self.goal, &self.opts, &self.cache)
    }

    /// Runs on tokio's blocking pool against `world` as it is while it runs.
    pub fn spawn_blocking(self, world: Arc<World>, registry: Arc<BlockRegistry>) -> Answer {
        let (tx, rx) = oneshot::channel();
        tokio::task::spawn_blocking(move || {
            // The receiver is gone when the goto was dropped or re-planned meanwhile.
            let terrain = Terrain::new(WorldBlocks::new(world), &registry).with_hazards(self.opts.hazards.clone());
            let _ = tx.send(self.run(&terrain));
        });
        rx
    }
}

/// An answer already known (inline searches, tests).
pub fn answered(path: Path) -> Answer {
    let (tx, rx) = oneshot::channel();
    let _ = tx.send(path);
    rx
}
