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
mod friend;
mod handle;
mod lan;
mod login;
mod net_wire;
mod nethernet;
mod pack_cache;
mod pack_fetch;
mod ping;
#[cfg(feature = "online")]
mod qos;
#[cfg(feature = "online")]
mod realm;
mod route;
mod signaling;
mod socks5;
#[cfg(test)]
mod test_host;
mod transport;
mod trickle;

pub use acacia_auth as auth;
pub use acacia_session::blob_store::{BlobStore, MemoryBlobStore};
pub use acacia_session::pack_store::{MemoryPackStore, PackStore};
pub use acacia_session::{proto, DisconnectReason};
pub use blob_cache::DiskBlobStore;
pub use pack_cache::DiskPackStore;
pub use client::{ClientBuilder, Login, TransportKind};
pub use handle::Client;
pub use acacia_nethernet::lan::ServerData as LanServerData;
pub use filter::PacketFilter;
pub use friend::FriendJoin;
#[cfg(feature = "online")]
pub use friend::{friend_builder, join_friend_world, FriendJoinError};
pub use lan::{discover_lan, LanServer};
pub use ping::{ping, ServerStatus};
#[cfg(feature = "online")]
pub use qos::measure_ping_regions;
#[cfg(feature = "online")]
pub use realm::{realm_builder, RealmJoinError};
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
    #[error("NetherNet needs the account's MultiplayerToken for its identity assertion; these credentials have none")]
    NetherNetNeedsOnline,
    #[error("the server does not answer NetherNet signaling (GET /v1/join)")]
    NetherNetUnsupported,
    #[error("signaling service: {0}")]
    Signaling(String),
}
