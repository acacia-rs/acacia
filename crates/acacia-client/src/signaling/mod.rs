//! Joins through the Minecraft signaling service (Realms, friends' worlds) over its WebSocket. Spec:
//! docs/research/nethernet-signaling.md §2; vanilla behaviour: docs/research/vanilla-realm-capture-2026-10-02.md.

mod keepalive;
#[cfg(test)]
mod tests;
mod ws;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub use acacia_nethernet::signaling::SignalingProtocol;
use acacia_nethernet::signaling::{SignalingEvent, SignalingSession, DEFAULT_PING_INTERVAL};
use acacia_nethernet::turn::IceServers;
use acacia_nethernet::{Identity, Signal};
use futures_util::StreamExt;
pub(crate) use keepalive::Keepalive;
use rand_core::{OsRng, RngCore};

use crate::net_wire::NetherNetWire;
use crate::socks5::Socks5Proxy;
use crate::transport::Transport;
use crate::trickle::{self, SignalPath};
use crate::ConnectError;

/// Offer, ICE and DTLS together; go-nethernet allows 15 s for the connection alone.
const DIAL_TIMEOUT: Duration = Duration::from_secs(30);

/// A join through the signaling service.
#[derive(Debug, Clone)]
pub struct SignalingTarget {
    /// E.g. `signaling-tm-<region>.franchise.minecraft-services.net`; a `ws://host:port` base skips TLS.
    pub host: String,
    pub protocol: SignalingProtocol,
    /// The host's id as the service names it (a realm's join `address`).
    pub peer: String,
    /// The MCToken: `ServiceToken::authorization_header`.
    pub mc_token: String,
}

#[cfg(feature = "online")]
impl SignalingTarget {
    /// The target for a NetherNet realm; None for RakNet realms (dial `join.address`) or unknown
    /// protocols. `host` is the signaling host from discovery (`SignalingEnvironment::service_uri`).
    pub fn from_realm(join: &acacia_auth::RealmJoin, mc_token: String, host: &str) -> Option<Self> {
        use acacia_auth::RealmProtocol as P;
        let protocol = match join.protocol {
            P::NetherNetJsonRpc => SignalingProtocol::JsonRpc,
            P::NetherNet => SignalingProtocol::Legacy,
            P::RakNet | P::Other(_) => return None,
        };
        let host = host.trim_start_matches("wss://").trim_start_matches("https://").trim_end_matches('/').to_owned();
        Some(Self { host, protocol, peer: join.address.clone(), mc_token })
    }
}

impl SignalingTarget {
    fn url(&self, own_id: u64) -> String {
        match self.host.strip_prefix("ws://") {
            Some(plain) => self.protocol.url(plain, own_id).replacen("wss://", "ws://", 1),
            None => self.protocol.url(&self.host, own_id),
        }
    }
}

/// What a finished dial hands the driver. The signaling socket stays open like vanilla's, held by
/// `keepalive` until the connection ends.
pub(crate) struct Dialed {
    pub transport: Transport,
    pub wire: NetherNetWire,
    /// The session's nominal remote: the signaling service's address.
    pub remote: SocketAddr,
    pub keepalive: Keepalive,
}

pub(crate) async fn dial(target: &SignalingTarget, identity: &Identity, proxy: Option<&Socks5Proxy>) -> Result<Dialed, ConnectError> {
    let deadline = Instant::now() + DIAL_TIMEOUT;
    let own_id = OsRng.next_u64();
    let headers = [
        ("authorization", target.mc_token.clone()),
        ("session-id", own_id.to_string()),
        ("request-id", OsRng.next_u64().to_string()),
    ];
    let (socket, remote) = ws::connect(&target.url(own_id), &headers, proxy).await?;
    let session = SignalingSession::new(target.protocol, own_id, DEFAULT_PING_INTERVAL, Instant::now());
    let mut path = ServicePath { socket, session, peer: &target.peer, ice_servers: None };
    let transport = Transport::bind(remote, proxy).await?;
    let wire = trickle::connect(&mut path, &transport, identity, deadline).await?;
    let keepalive = Keepalive::spawn(path.socket, path.session);
    Ok(Dialed { transport, wire, remote, keepalive })
}

struct ServicePath<'a> {
    socket: ws::Socket,
    session: SignalingSession,
    peer: &'a str,
    ice_servers: Option<IceServers>,
}

impl SignalPath for ServicePath<'_> {
    fn send(&mut self, signal: Signal) {
        self.session.send_signal(self.peer, &signal, Instant::now());
    }

    async fn flush(&mut self) -> Result<(), ConnectError> {
        keepalive::flush(&mut self.socket, &mut self.session).await
    }

    async fn recv(&mut self) -> Result<Vec<Signal>, ConnectError> {
        keepalive::handle_frame(&mut self.session, self.socket.next().await)?;
        let mut signals = Vec::new();
        while let Some(event) = self.session.poll_event() {
            match event {
                SignalingEvent::Signal { signal, .. } => signals.push(signal),
                SignalingEvent::Credentials(servers) => self.ice_servers = Some(servers),
                SignalingEvent::Error(e) => tracing::debug!("signaling: {e}"),
            }
        }
        Ok(signals)
    }

    fn poll_timeout(&self) -> Option<Instant> {
        Some(self.session.poll_timeout())
    }

    fn handle_timeout(&mut self, now: Instant) {
        self.session.handle_timeout(now);
    }

    fn take_ice_servers(&mut self) -> Option<IceServers> {
        self.ice_servers.take()
    }
}
