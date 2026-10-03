//! Async Minecraft: Bedrock Edition client: tokio driver for `acacia-session`, SOCKS5 support and
//! the public connection API.
//!
//! ```no_run
//! # async fn run() -> Result<(), acacia_client::ConnectError> {
//! use acacia_client::{proto::packets::SetTime, proto::Packet, Client, Event};
//! let mut client = Client::builder("127.0.0.1:19132").offline("Bot").subscribe([SetTime::ID]).connect().await?;
//! while let Some(Event::Packet(p)) = client.recv().await {
//!     let time: SetTime = p.decode().unwrap();
//!     println!("{}", time.time);
//! }
//! # Ok(()) }
//! ```

mod actions;
mod blob_cache;
mod client;
mod driver;
mod filter;
mod lan;
mod login;
mod net_wire;
mod nethernet;
mod ping;
#[cfg(feature = "online")]
mod qos;
mod route;
mod signaling;
mod socks5;
#[cfg(test)]
mod test_host;
mod transport;
mod trickle;

pub use acacia_auth as auth;
pub use acacia_session::blob_store::{BlobStore, MemoryBlobStore};
pub use acacia_session::{proto, DisconnectReason};
pub use blob_cache::DiskBlobStore;
pub use client::{Client, ClientBuilder, Login, TransportKind};
pub use acacia_nethernet::lan::ServerData as LanServerData;
pub use filter::PacketFilter;
pub use lan::{discover_lan, LanServer};
pub use ping::{ping, ServerStatus};
#[cfg(feature = "online")]
pub use qos::measure_ping_regions;
pub use signaling::{SignalingProtocol, SignalingTarget};
pub use socks5::{ProxyParseError, Socks5Proxy};

#[derive(Debug)]
pub enum Event {
    Packet(proto::RawPacket),
    /// Always the last event.
    Disconnected(DisconnectReason),
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("could not resolve server address")]
    Resolve,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("disconnected during login: {0:?}")]
    Disconnected(DisconnectReason),
    #[error("timed out waiting for the server")]
    Timeout,
    #[error("NetherNet: {0}")]
    NetherNet(#[from] acacia_nethernet::Error),
    #[error("NetherNet needs an online login (its identity assertion carries the MultiplayerToken)")]
    NetherNetNeedsOnline,
    #[error("the server does not answer NetherNet signaling (GET /v1/join)")]
    NetherNetUnsupported,
    #[error("signaling service: {0}")]
    Signaling(String),
}
