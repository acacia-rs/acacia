//! High-level Bedrock bot on top of `acacia-client`: tracked game state now; physics,
//! interactions and pathfinding follow (see docs/DESIGN.md).
//!
//! Each [`Bot`] owns its state and updates it as packets arrive in [`Bot::next`], so there is no
//! locking and no background work beyond the client's own connection task.

pub mod books;
mod bot;
mod cadence;
mod config;
mod elytra;
pub mod events;
mod fishing;
pub mod forms;
mod human;
pub mod interact;
pub mod items;
pub mod movement;
pub mod pathfind;
mod prediction_sync;
mod reflex;
mod respawn;
pub mod signs;
mod sleep;
mod riding;
mod spawn;
mod subchunks;
pub mod state;
pub mod survival;
mod wait;
pub mod workstation;
pub mod world;

pub use wait::ActionError;
pub mod text;
pub mod trace;

pub use acacia_client::{self as client, proto, ConnectError, DisconnectReason};
pub use bot::Bot;
pub use config::BotConfig;
pub use events::{BotEvent, Events};
pub use state::GameState;
