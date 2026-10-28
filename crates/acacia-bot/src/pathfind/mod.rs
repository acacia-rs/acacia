//! Pathfinding after azalea's design (docs/research/03-azalea-architecture.md §5): Baritone-style A*
//! over feet block positions with costs in ticks and a best-partial-path fallback ([`search`]),
//! and a per-tick follower that drives [`Controls`](crate::movement::Controls) and re-plans when
//! stuck, knocked off the path, teleported or when blocks on the path change ([`Bot::goto`],
//! [`Navigator`]).
//!
//! Opt-in moves change the world first ([`Work`]): digging through blocks, bridging gaps and
//! pillaring up with scaffold blocks ([`SearchOpts::allow_dig`], [`SearchOpts::allow_bridge`]);
//! opening wooden doors and gates is on by default. The navigator performs them with the bot's
//! break, place and use actions.
//!
//! The search is synchronous and reads blocks through [`Blocks`], so it runs against a
//! [`ChunkView`](acacia_world::ChunkView), a shared world snapshot ([`WorldBlocks`], used on a
//! blocking thread for long searches) or a synthetic grid in tests.

mod alter;
mod astar;
pub mod boat;
mod bot_goto;
mod cache;
mod cell;
mod control;
mod costs;
mod execute;
mod exit;
mod favour;
pub mod flight;
mod follow;
mod follow_macro;
mod frontier;
mod goal;
mod goto;
mod hop;
mod job;
mod judge;
mod kit;
mod leap;
mod leap_moves;
mod mac;
mod macros;
mod maneuver;
mod memo;
mod moves;
mod node;
mod opts;
mod physview;
mod pilot;
pub mod process;
mod profile;
mod reach;
mod retry;
mod rollout;
mod shift;
mod shortcut;
mod snapshot;
mod solve;
mod terrain;
mod work;

#[cfg(test)]
mod tests;

pub use astar::{Path, SearchOpts, search, search_cached, search_from, start_node};
pub use cache::RolloutCache;
pub use macros::MacroStats;
pub use control::{Controller, Phase, RunCtl};
pub use mac::Macro;
pub use shortcut::ShortcutCtl;
pub use leap::{JumpAt, LeapCtl};
pub use follow::{FollowStatus, Follower, ROLLOUT_BUDGET, Sense};
pub use favour::Favour;
pub use follow_macro::MacroReport;
pub use goal::Goal;
pub use goto::{GotoOpts, NavStatus, Navigator};
pub use job::{Answer, SearchJob, Spawn, answered};
pub use pilot::Pilot;
pub use process::{Backfill, Follow, Goto, Processes, Target};
pub use kit::{DEFAULT_SCAFFOLD, Kit};
pub use moves::MoveKind;
pub use node::{Entry, Mode, Momentum, NodeKey, speed_bin};
pub use profile::{Aim, MovementProfile, Perturbation};
pub use rollout::{Body, Spend, Trace, peek, run};
pub use snapshot::WorldBlocks;
pub use terrain::{Blocks, Cell, Spot, SpotKind, Terrain};
pub use work::{Step, Work};

use acacia_physics::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathNode {
    pub pos: BlockPos,
    /// Feet height when standing at the node.
    pub feet: f32,
    /// The move that reaches this node from the previous one.
    pub kind: MoveKind,
    pub wet: bool,
    /// World changes the move needs first, done from the previous node.
    pub work: Work,
    /// The motion the node is reached with.
    pub entry: Entry,
    /// That motion as planned (the standing one for `Entry::Rest`).
    pub mom: Momentum,
    /// The controller that reaches the node from the previous one, for macro moves.
    pub mac: Option<Macro>,
    /// The move's planned cost: ticks (simulated for macro moves) plus the profile's penalties.
    pub cost: f32,
}
