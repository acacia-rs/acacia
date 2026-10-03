//! LAN worlds: discovery broadcasts on UDP 7551, and signals carried in its Message packets.
//! Spec: docs/research/nethernet-signaling.md §1.

mod socket;

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use acacia_nethernet::lan::{LanPacket, ServerData};
use acacia_nethernet::turn::IceServers;
use acacia_nethernet::{Identity, Signal};
use rand_core::{OsRng, RngCore};

use self::socket::LanSocket;

use crate::net_wire::NetherNetWire;
use crate::transport::Transport;
use crate::trickle::{self, SignalPath};
use crate::ConnectError;

/// Vanilla's cadence (qos-lan-capture-2026-10-03.md).
const REQUEST_INTERVAL: Duration = Duration::from_secs(2);
/// No answer within 15 s is CONNECTERROR 14 for go-nethernet; ICE and DTLS get the same again.
const DIAL_TIMEOUT: Duration = Duration::from_secs(30);

/// A world that answered discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanServer {
    /// Where its discovery answer came from; signals go here.
    pub addr: SocketAddr,
    /// The host's NetherNet id (the answer's sender id).
    pub network_id: u64,
    pub data: ServerData,
    /// Our NetherNet id during discovery; the dial keeps it, as vanilla keeps one id per session.
    pub local_id: u64,
}

/// Broadcasts discovery requests for `duration` and returns every world that answered.
pub async fn discover_lan(duration: Duration) -> io::Result<Vec<LanServer>> {
    let socket = LanSocket::bind()?;
    let own_id = OsRng.next_u64();
    let request = LanPacket::Request.encode(own_id);
    let end = Instant::now() + duration;
    let mut found: HashMap<u64, LanServer> = HashMap::new();
    let mut buf = vec![0u8; 2048];
    let mut domain: Option<Vec<SocketAddr>> = None;
    let mut next_request = Instant::now();
    while Instant::now() < end {
        if Instant::now() >= next_request {
            broadcast_request(&socket, &request, &mut domain).await?;
            next_request = Instant::now() + REQUEST_INTERVAL;
        }
        let wake = next_request.min(end);
        tokio::select! {
            r = socket.recv_from(&mut buf) => {
                let (n, addr) = match r {
                    Ok(r) => r,
                    // Windows reports ICMP port-unreachable on UDP sockets as a reset.
                    Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                    Err(e) => return Err(e),
                };
                match LanPacket::decode(&buf[..n]) {
                    Ok((LanPacket::Response(bytes), sender)) if sender != own_id => match ServerData::decode(&bytes) {
                        Ok(data) => {
                            found.insert(sender, LanServer { addr, network_id: sender, data, local_id: own_id });
                        }
                        Err(e) => tracing::debug!(%addr, "bad LAN ServerData: {e}"),
                    },
                    Ok(_) => {}
                    Err(e) => tracing::trace!(%addr, "not a LAN packet: {e}"),
                }
            }
            _ = tokio::time::sleep_until(wake.into()) => {}
        }
    }
    Ok(found.into_values().collect())
}

/// The first request goes to the fallback (vanilla's interface scan is still running); later ones to
/// the broadcast domain, which loses any address a send fails on.
async fn broadcast_request(socket: &LanSocket, request: &[u8], domain: &mut Option<Vec<SocketAddr>>) -> io::Result<()> {
    let Some(addrs) = domain else {
        socket.send_to(request, socket::FALLBACK).await?;
        let addrs = socket.broadcast_domain()?;
        tracing::debug!(?addrs, "LAN broadcast domain");
        *domain = Some(addrs);
        return Ok(());
    };
    let mut failed = Vec::new();
    for &addr in addrs.iter() {
        if let Err(e) = socket.send_to(request, addr).await {
            tracing::debug!(%addr, "removing from LAN broadcast domain: {e}");
            failed.push(addr);
        }
    }
    addrs.retain(|a| !failed.contains(a));
    Ok(())
}

/// Connects to a discovered world: signals over discovery packets, ICE on its own socket.
pub(crate) async fn dial(server: &LanServer, identity: &Identity) -> Result<(Transport, NetherNetWire), ConnectError> {
    let deadline = Instant::now() + DIAL_TIMEOUT;
    let mut path = LanPath { socket: LanSocket::bind()?, own_id: server.local_id, server, outbox: Vec::new() };
    let transport = Transport::bind(server.addr, None).await?;
    let wire = trickle::connect(&mut path, &transport, identity, deadline).await?;
    Ok((transport, wire))
}

struct LanPath<'a> {
    socket: LanSocket,
    own_id: u64,
    server: &'a LanServer,
    outbox: Vec<Signal>,
}

impl SignalPath for LanPath<'_> {
    fn send(&mut self, signal: Signal) {
        self.outbox.push(signal);
    }

    async fn flush(&mut self) -> Result<(), ConnectError> {
        for signal in std::mem::take(&mut self.outbox) {
            let packet = LanPacket::Message { recipient: self.server.network_id, data: signal.to_string() };
            self.socket.send_to(&packet.encode(self.own_id), self.server.addr).await?;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<Signal>, ConnectError> {
        let mut buf = [0u8; 4096];
        let n = match self.socket.recv_from(&mut buf).await {
            Ok((n, _)) => n,
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => return Ok(vec![]),
            Err(e) => return Err(e.into()),
        };
        Ok(match LanPacket::decode(&buf[..n]) {
            Ok((LanPacket::Message { recipient, data }, sender))
                if recipient == self.own_id && sender == self.server.network_id && !data.is_empty() && data != "Ping" =>
            {
                Signal::parse(&data).map_err(|e| tracing::debug!("bad LAN signal: {e}")).into_iter().collect()
            }
            _ => vec![],
        })
    }

    fn poll_timeout(&self) -> Option<Instant> {
        None
    }

    fn handle_timeout(&mut self, _: Instant) {}

    /// LAN has no TURN (go-nethernet `Credentials()` is nil).
    fn take_ice_servers(&mut self) -> Option<IceServers> {
        None
    }

    /// A vanilla-hosted world answered without `a=identity` (live test 2026-10-02).
    fn identityless_host_ok(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use p384::ecdsa::SigningKey;
    use tokio::net::UdpSocket;

    use super::*;
    use crate::test_host::RtcHost;

    const HOST_ID: u64 = 77;

    /// A LAN host on loopback: answers our discovery-channel Messages through a str0m peer.
    async fn run_host(discovery: UdpSocket, mut rtc: RtcHost) {
        let mut buf = [0u8; 4096];
        loop {
            tokio::select! {
                r = discovery.recv_from(&mut buf) => {
                    let (n, from) = r.unwrap();
                    let Ok((LanPacket::Message { recipient, data }, sender)) = LanPacket::decode(&buf[..n]) else { continue };
                    assert_eq!(recipient, HOST_ID);
                    for out in rtc.on_signal(&Signal::parse(&data).unwrap()) {
                        let reply = LanPacket::Message { recipient: sender, data: out.to_string() };
                        discovery.send_to(&reply.encode(HOST_ID), from).await.unwrap();
                    }
                }
                _ = rtc.step() => {}
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dials_a_lan_host_over_discovery_messages() {
        let discovery = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server = LanServer { addr: discovery.local_addr().unwrap(), network_id: HOST_ID, data: ServerData::default(), local_id: 7 };
        // Like the vanilla game hosting a world: no a=identity in the answer.
        tokio::spawn(run_host(discovery, RtcHost::bind(false).await));
        let identity = Identity::multiplayer(SigningKey::from_slice(&[5; 48]).unwrap(), "token".into());
        let dialed = tokio::time::timeout(Duration::from_secs(20), dial(&server, &identity)).await;
        let (_, wire) = dialed.expect("dial timed out").expect("dial failed");
        assert!(wire.conn.is_open());
    }
}
