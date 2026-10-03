use std::time::Duration;

use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct Config {
    pub guid: u64,
    pub protocol_version: u8,
    /// Tried largest first; each size gets a few attempts before falling back.
    pub mtu_sizes: Vec<u16>,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub ping_interval: Duration,
}

impl Config {
    pub fn new(guid: u64) -> Self {
        Self {
            guid,
            protocol_version: 11,
            mtu_sizes: vec![1492, 1200, 576],
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(10),
            ping_interval: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
    ConnectTimeout,
    Timeout,
    ServerClosed,
    /// A server's peer sent a disconnection notification.
    ClientClosed,
    LocalClose,
    IncompatibleProtocol,
    AlreadyConnected,
    ServerFull,
    Banned,
    RecentlyConnected,
    Protocol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connected { mtu: u16 },
    /// A message from the server (the first byte is its ID; Bedrock game batches start with 0xfe).
    Message(Bytes),
    Disconnected(DisconnectReason),
}
