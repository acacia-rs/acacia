//! Network-free Bedrock session layer: batching, compression, encryption and the login handshake
//! on top of a `acacia-raknet` connection or a message transport (NetherNet).

pub mod batch;
pub mod blob_store;
pub mod compression;
pub mod crypto;
mod error;
mod session;

pub use acacia_proto as proto;
pub use acacia_raknet as raknet;
pub use error::Error;
pub use session::{DisconnectReason, Event, LinkConfig, Session, SessionConfig};
