use std::net::SocketAddr;
use std::time::{Duration, Instant};

use acacia_session::raknet;
use bytes::BytesMut;

use crate::socks5::Socks5Proxy;
use crate::transport::{raknet_client_guid, resolve, Transport};
use crate::ConnectError;

const ATTEMPTS: u32 = 3;
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);

/// A server's advertised status (RakNet unconnected pong).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerStatus {
    pub edition: String,
    pub motd: String,
    pub protocol: i32,
    pub version: String,
    pub players_online: u32,
    pub players_max: u32,
    pub server_id: String,
    pub sub_motd: String,
    pub game_mode: String,
    pub latency: Duration,
    /// The full `;`-separated advertisement, for fields not parsed above.
    pub raw: String,
}

impl ServerStatus {
    fn parse(raw: String, latency: Duration) -> Self {
        let f: Vec<&str> = raw.split(';').collect();
        let s = |i: usize| f.get(i).copied().unwrap_or_default().to_owned();
        fn n<T: std::str::FromStr + Default>(f: &[&str], i: usize) -> T {
            f.get(i).and_then(|v| v.parse().ok()).unwrap_or_default()
        }
        Self {
            edition: s(0),
            motd: s(1),
            protocol: n(&f, 2),
            version: s(3),
            players_online: n(&f, 4),
            players_max: n(&f, 5),
            server_id: s(6),
            sub_motd: s(7),
            game_mode: s(8),
            latency,
            raw,
        }
    }
}

/// Pings a server (optionally through a SOCKS5 proxy) without logging in.
pub async fn ping(server: &str, proxy: Option<&Socks5Proxy>) -> Result<ServerStatus, ConnectError> {
    ping_addr(resolve(server).await?, proxy).await
}

pub(crate) async fn ping_addr(addr: SocketAddr, proxy: Option<&Socks5Proxy>) -> Result<ServerStatus, ConnectError> {
    let transport = Transport::connect(addr, proxy).await?;
    let guid = raknet_client_guid();
    let mut buf = vec![0u8; 2048];
    for _ in 0..ATTEMPTS {
        let mut ping = BytesMut::with_capacity(33);
        raknet::unconnected_ping(&mut ping, 0, guid);
        let sent = Instant::now();
        transport.send(&ping).await?;
        let deadline = tokio::time::Instant::now() + ATTEMPT_TIMEOUT;
        while let Ok(received) = tokio::time::timeout_at(deadline, transport.recv_from(&mut buf)).await {
            if let Ok(pong) = raknet::parse_pong(&buf[received?.0]) {
                return Ok(ServerStatus::parse(pong.motd, sent.elapsed()));
            }
        }
    }
    Err(ConnectError::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bedrock_advertisement() {
        let s = ServerStatus::parse("MCPE;Example Network;2193;26.51;16569;50000;123;Sub;Survival;1;19132;19133;".into(), Duration::ZERO);
        assert_eq!((s.motd.as_str(), s.protocol, s.players_online, s.players_max), ("Example Network", 2193, 16569, 50000));
        assert_eq!(s.game_mode, "Survival");
        assert_eq!(ServerStatus::parse("MCPE;x".into(), Duration::ZERO).players_max, 0);
    }
}
