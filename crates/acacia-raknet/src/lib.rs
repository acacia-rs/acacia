//! Network-free RakNet for Minecraft: Bedrock Edition.
//!
//! [`Client`] and [`Server`] never touch a socket: feed them received datagrams and timer expiries,
//! then drain datagrams to send and events. See `acacia-client` for the tokio driver.

mod client;
mod conn;
mod reliability;
mod server;
mod types;
pub mod wire;

pub use client::Client;
pub use server::{PeerStats, Server, ServerConfig, ServerEvent};
pub use types::{Config, DisconnectReason, Event};
pub use reliability::{RecvError, RecvLimits};
pub use wire::datagram::Reliability;
pub use wire::offline::{parse_pong, unconnected_ping, Pong};
pub use wire::WireError;
