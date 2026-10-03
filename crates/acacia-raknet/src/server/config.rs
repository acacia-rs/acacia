use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;

use super::cookie::random_secret;
use crate::reliability::RecvLimits;
use crate::types::DisconnectReason;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub guid: u64,
    /// Answer to unconnected pings (`MCPE;...`); see [`super::Server::set_motd`].
    pub motd: String,
    pub protocol_version: u8,
    pub max_mtu: u16,
    pub idle_timeout: Duration,
    pub ping_interval: Duration,
    pub recv_limits: RecvLimits,
    /// Make clients prove their address with a cookie before a connection exists (`cookie.rs`).
    /// Vanilla and BDS do this; turn it off only for clients that cannot echo one.
    pub cookies: bool,
    /// Keys the cookies; random per `ServerConfig::new`, so cookies do not outlive a restart.
    pub cookie_secret: u128,
    /// Peers held at once, handshaking ones included; a request 2 beyond it is told the server is full.
    pub max_peers: usize,
    /// Time a peer gets from reply 2 to its NewIncomingConnection, however busily it keeps talking.
    pub handshake_timeout: Duration,
}

impl ServerConfig {
    pub fn new(guid: u64, motd: String) -> Self {
        Self {
            guid,
            motd,
            protocol_version: 11,
            max_mtu: 1492,
            idle_timeout: Duration::from_secs(10),
            ping_interval: Duration::from_secs(2),
            recv_limits: RecvLimits::SERVER,
            cookies: true,
            cookie_secret: random_secret(),
            max_peers: 1024,
            handshake_timeout: Duration::from_secs(5),
        }
    }
}

/// A snapshot of one peer, from [`super::Server::stats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerStats {
    pub guid: u64,
    pub mtu: u16,
    /// Smoothed round trip of our datagrams to their ACKs; `None` until the first ACK.
    pub rtt: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// `guid` is what the client claimed in request 2; nothing makes it unique or stable.
    Connected { addr: SocketAddr, guid: u64, mtu: u16 },
    /// A message from a connected peer (the first byte is its ID; Bedrock game batches start with 0xfe).
    Message(SocketAddr, Bytes),
    Disconnected(SocketAddr, DisconnectReason),
}
