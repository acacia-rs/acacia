//! Picks RakNet or NetherNet for a join and opens it.

use std::net::SocketAddr;

use acacia_session::{raknet, LinkConfig};
use acacia_nethernet::Identity;

use crate::client::TransportKind;
use crate::driver::Wire;
use crate::lan::{self, LanServer};
use crate::net_wire::NetherNetWire;
use crate::signaling::{self, Keepalive, SignalingTarget};
use crate::nethernet::{self, Endpoint, Scheme};
use crate::ping::ping_addr;
use crate::socks5::Socks5Proxy;
use crate::transport::{raknet_client_guid, with_port, Transport};
use crate::ConnectError;

/// An open path to the server, ready for a session.
pub(crate) struct Route {
    pub transport: Transport,
    pub wire: Wire,
    pub link: LinkConfig,
    /// What the Login client data reports as `ServerAddress`.
    pub server_address: String,
    /// The server's address, or the signaling service's for a signaled join.
    pub remote: SocketAddr,
    /// A signaled join's socket, kept open for the session.
    pub keepalive: Option<Keepalive>,
}

/// A LAN world found by discovery; always NetherNet.
pub(crate) async fn open_lan(server: &LanServer, identity: Option<&Identity>) -> Result<Route, ConnectError> {
    let identity = identity.ok_or(ConnectError::NetherNetNeedsOnline)?;
    let (transport, wire) = lan::dial(server, identity).await?;
    Ok(Route {
        transport,
        wire: Wire::NetherNet(Box::new(wire)),
        link: LinkConfig::Message,
        // TODO: capture the ServerAddress vanilla sends for LAN joins.
        server_address: server.addr.to_string(),
        remote: server.addr,
        keepalive: None,
    })
}

/// A join through the signaling service; always NetherNet.
pub(crate) async fn open_signaling(
    target: &SignalingTarget,
    proxy: Option<&Socks5Proxy>,
    identity: Option<&Identity>,
) -> Result<Route, ConnectError> {
    let identity = identity.ok_or(ConnectError::NetherNetNeedsOnline)?;
    let dialed = signaling::dial(target, identity, proxy).await?;
    Ok(Route {
        transport: dialed.transport,
        wire: Wire::NetherNet(Box::new(dialed.wire)),
        link: LinkConfig::Message,
        // TODO: capture the ServerAddress vanilla sends for realm joins.
        server_address: target.peer.clone(),
        remote: dialed.remote,
        keepalive: Some(dialed.keepalive),
    })
}

enum Choice {
    RakNet,
    /// With the signaling scheme if a probe already found it.
    NetherNet(Option<Scheme>),
}

/// `identity` is None for an online login that came without a MultiplayerToken; NetherNet needs one.
pub(crate) async fn open(
    server: &str,
    addr: SocketAddr,
    proxy: Option<&Socks5Proxy>,
    kind: TransportKind,
    identity: Option<&Identity>,
) -> Result<Route, ConnectError> {
    let with_port = with_port(server);
    let host = with_port.rsplit_once(':').map_or(server, |(host, _)| host);
    let endpoint = Endpoint { addr, host, proxy };
    let choice = match (kind, identity) {
        (TransportKind::Auto, Some(_)) => choose(&endpoint).await,
        (TransportKind::Auto, None) | (TransportKind::RakNet, _) => Choice::RakNet,
        (TransportKind::NetherNet, _) => Choice::NetherNet(None),
    };
    match choice {
        Choice::RakNet => Ok(Route {
            transport: Transport::connect(addr, proxy).await?,
            wire: Wire::RakNet,
            link: LinkConfig::RakNet(raknet::Config::new(raknet_client_guid())),
            // Geyser splits ServerAddress on ':' and kicks if the port is missing.
            server_address: with_port,
            remote: addr,
            keepalive: None,
        }),
        Choice::NetherNet(scheme) => {
            let identity = identity.ok_or(ConnectError::NetherNetNeedsOnline)?;
            let (transport, conn, scheme) = nethernet::dial(&endpoint, scheme, identity).await?;
            Ok(Route {
                transport,
                wire: Wire::NetherNet(Box::new(NetherNetWire::new(conn, None))),
                link: LinkConfig::Message,
                server_address: endpoint.server_address(scheme),
                remote: addr,
                keepalive: None,
            })
        }
    }
}

/// RakNet as soon as a ping answers; NetherNet only if RakNet stays silent and the probe succeeds.
async fn choose(endpoint: &Endpoint<'_>) -> Choice {
    let ping = ping_addr(endpoint.addr, endpoint.proxy);
    let probe = nethernet::probe(endpoint);
    tokio::pin!(ping, probe);
    let mut probed = None;
    loop {
        tokio::select! {
            pong = &mut ping => {
                if pong.is_ok() {
                    return Choice::RakNet;
                }
                let scheme = match probed { Some(scheme) => scheme, None => probe.await };
                return scheme.map_or(Choice::RakNet, |s| Choice::NetherNet(Some(s)));
            }
            scheme = &mut probe, if probed.is_none() => probed = Some(scheme),
        }
    }
}
