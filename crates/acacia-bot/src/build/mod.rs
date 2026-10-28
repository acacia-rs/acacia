//! Schematic building: a `.mcstructure` placed block-exact by a [`Builder`] process, which re-surveys the world
//! every tick (so a restart resumes from what is there), restocks from stations by remaining need, and cleans
//! up its scaffold. Several builders can share a job through a [`TaskBoard`]. Design: acacia-pathfind/docs/builder.md.

mod board;
mod builder;
mod check;
mod cover;
mod en_route;
pub mod findings;
mod job;
pub mod journal;
mod pairs;
mod plan;
mod process;
mod reach;
mod restock;
mod rules;
mod status;
mod structure;
mod survey;
mod swarm;
mod transform;
#[cfg(test)]
pub(crate) mod tests;

pub use board::{LocalBoard, TaskBoard};
pub use builder::{BuildError, Builder};
pub use check::{check, Mismatch};
pub use job::BuildJob;
pub use rules::supported;
pub use status::{BuildStatus, HoldWhy, Phase, StatusHandle};
pub use structure::{Block, Structure, StructureError};
pub use transform::Transform;
