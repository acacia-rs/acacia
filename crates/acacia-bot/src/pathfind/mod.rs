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
mod cell;
mod costs;
mod execute;
mod follow;
mod goal;
mod goto;
mod kit;
mod maneuver;
mod moves;
mod snapshot;
mod terrain;
mod work;

#[cfg(test)]
mod tests;

pub use astar::{Path, SearchOpts, search, start_node};
pub use follow::{FollowStatus, Follower, Sense};
pub use goal::Goal;
pub use goto::{GotoOpts, NavStatus, Navigator};
pub use kit::{DEFAULT_SCAFFOLD, Kit};
pub use moves::MoveKind;
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
}
