//! Pathfinding after azalea's design (docs/research/03-azalea-architecture.md §5): Baritone-style A*
//! over feet block positions with costs in ticks and a best-partial-path fallback ([`search`]),
//! and a per-tick follower that drives [`Controls`](crate::movement::Controls) and re-plans when
//! stuck, knocked off the path, teleported or when blocks on the path change ([`Bot::goto`],
//! [`Navigator`]).
//!
//! The search is synchronous and reads blocks through [`Blocks`], so it runs against a
//! [`ChunkView`](acacia_world::ChunkView), a shared world snapshot ([`WorldBlocks`], used on a
//! blocking thread for long searches) or a synthetic grid in tests.

mod astar;
mod costs;
mod follow;
mod goal;
mod goto;
mod moves;
mod snapshot;
mod terrain;

#[cfg(test)]
mod tests;

pub use astar::{Path, SearchOpts, search, start_node};
pub use follow::{FollowStatus, Follower, Sense};
pub use goal::Goal;
pub use goto::{GotoOpts, NavStatus, Navigator};
pub use moves::MoveKind;
pub use snapshot::WorldBlocks;
pub use terrain::{Blocks, Cell, Spot, SpotKind, Terrain};

use acacia_physics::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathNode {
    pub pos: BlockPos,
    /// Feet height when standing at the node.
    pub feet: f32,
    /// The move that reaches this node from the previous one.
    pub kind: MoveKind,
    pub wet: bool,
}
