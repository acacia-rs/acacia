use std::sync::{Arc, Mutex};

use acacia_physics::BlockPos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Phase {
    #[default]
    Surveying,
    Building,
    Restocking,
    /// Removing scaffold once every cell is right.
    Cleaning,
    /// Nothing doable: missing materials no station has, or unreachable cells.
    Waiting,
    Done,
}

/// Why a builder held still for a tick (journal lines, `build::findings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HoldWhy {
    /// Off the ground with no drive to keep: waiting to land.
    InAir,
    /// The stand just reached gave no click after all.
    ArrivedNoClick,
    /// Another bot claimed the cell first.
    ClaimLost,
    /// Placements in flight still to be decided.
    InFlight,
    /// Items no station has.
    Missing,
    /// Work is left but none of it can be done now (others' claims, nothing in reach or supported).
    Blocked,
    /// Scaffold stands but none of it can be reached now.
    Cleaning,
    /// Given-up cells were revived for another try.
    Revive,
    /// At a station another bot has open, or the open is under way.
    Station,
}

/// A builder's progress, readable while it runs through [`StatusHandle`].
#[derive(Debug, Clone, Default)]
pub struct BuildStatus {
    pub phase: Phase,
    /// Footprint cells with a target (structure void excluded).
    pub cells: usize,
    pub done: usize,
    /// Cells to place, or to use or write (`Need::Adjust`).
    pub to_place: usize,
    pub to_break: usize,
    /// Not loaded yet.
    pub unknown: usize,
    /// Fill-only jobs: cells holding another block, left alone.
    pub kept: usize,
    /// Targets outside the builder's block set, skipped.
    pub unsupported: Vec<(BlockPos, String)>,
    /// Cells given up on after repeated failures, with why ("acts failed", "placed wrong", "no stand").
    pub failed: Vec<(BlockPos, &'static str)>,
    /// Temporary blocks still standing.
    pub scaffold: usize,
    /// Seals left in place outside the footprint, holding liquids back.
    pub seals: Vec<BlockPos>,
    /// Items still needed that neither the inventory nor any station has.
    pub missing: Vec<String>,
    pub placed: u32,
    pub broken: u32,
    pub restocks: u32,
    /// Use-clicks and sign sides written.
    pub adjusted: u32,
    /// Goals the pilot was given (a goal kept across ticks counts once).
    pub walks: u32,
}

pub type StatusHandle = Arc<Mutex<BuildStatus>>;
