//! The host's own srflx and relay candidates: a STUN binding per STUN server, kept fresh, and one
//! TURN allocation per TURN server shared by every peer (permissions and channels per peer address).
//! Peers get them in their answers; the STUN/TURN traffic shares the host's socket.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::turn::stun::{self, TransactionId};
use crate::turn::{self, Retransmit, Step, TurnClient, TurnEvent};
use crate::LocalCandidate;

/// Rebinding keeps the NAT mapping the srflx candidate names alive (libwebrtc's STUN keepalive is 10 s).
const STUN_KEEPALIVE: Duration = Duration::from_secs(10);
/// A STUN server that never answered is asked again after this long.
const STUN_RETRY: Duration = Duration::from_secs(60);
/// A failed or lost allocation is retried after this long.
const TURN_RETRY: Duration = Duration::from_secs(60);

/// STUN and TURN servers to gather candidates from, already resolved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IceConfig {
    pub stun: Vec<SocketAddr>,
    pub turn: Vec<TurnServer>,
}

/// A UDP TURN server and its long-term credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnServer {
    pub addr: SocketAddr,
    pub username: String,
    pub password: String,
}

struct Stun {
    server: SocketAddr,
    request: Option<(TransactionId, Vec<u8>, Retransmit)>,
    next_at: Option<Instant>,
    mapped: Option<SocketAddr>,
}

struct Relay {
    server: TurnServer,
    client: Option<TurnClient>,
    mapped: Option<SocketAddr>,
    retry_at: Option<Instant>,
}

/// What a datagram from a STUN or TURN server was.
pub(super) enum Claim {
    Consumed,
    /// Peer data unwrapped from the allocation at `relay`.
    Relayed { relay: SocketAddr, peer: SocketAddr, data: Vec<u8> },
}

#[derive(Default)]
pub(super) struct Gather {
    stun: Vec<Stun>,
    relays: Vec<Relay>,
    transmits: VecDeque<(SocketAddr, Vec<u8>)>,
}

impl Gather {
    pub fn new(config: IceConfig) -> Self {
        let stun = config.stun.into_iter().map(|server| Stun { server, request: None, next_at: None, mapped: None }).collect();
        let relays = config.turn.into_iter().map(|server| Relay { server, client: None, mapped: None, retry_at: None }).collect();
        Self { stun, relays, transmits: VecDeque::new() }
    }

    pub fn start(&mut self, now: Instant) {
        for stun in &mut self.stun {
            stun.next_at = Some(now);
        }
        for relay in &mut self.relays {
            relay.retry_at = Some(now);
        }
        self.handle_timeout(now);
    }

    /// The srflx and relay candidates to offer beside the host candidate `host`.
    pub fn candidates(&self, host: SocketAddr) -> Vec<LocalCandidate> {
        let mut out = Vec::new();
        let mapped = self.stun.iter().filter_map(|s| s.mapped).chain(self.relays.iter().filter_map(|r| r.mapped));
        for addr in mapped.filter(|addr| *addr != host && addr.is_ipv4() == host.is_ipv4()) {
            let candidate = LocalCandidate::ServerReflexive { addr, base: host };
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
        for relay in &self.relays {
            if let Some(addr) = relay.client.as_ref().and_then(TurnClient::relayed) {
                out.push(LocalCandidate::Relayed { addr, mapped: relay.mapped.unwrap_or(host) });
            }
        }
        out
    }

    pub fn handle_datagram(&mut self, now: Instant, from: SocketAddr, data: &[u8]) -> Option<Claim> {
        for stun in self.stun.iter_mut().filter(|s| s.server == from) {
            let Some(mapped) = stun.request.as_ref().and_then(|(tid, _, _)| turn::parse_binding_response(tid, data)) else { continue };
            if stun.mapped != Some(mapped) {
                tracing::debug!(server = %from, %mapped, "STUN mapped address");
            }
            (stun.mapped, stun.request, stun.next_at) = (Some(mapped), None, Some(now + STUN_KEEPALIVE));
            return Some(Claim::Consumed);
        }
        let Some(relay) = self.relays.iter_mut().find(|r| r.client.as_ref().is_some_and(|c| c.server() == from)) else {
            // A late answer to a retransmitted binding.
            return (self.stun.iter().any(|s| s.server == from) && stun::is_stun(data)).then_some(Claim::Consumed);
        };
        let client = relay.client.as_mut().expect("found by its client");
        let relayed = client.handle_datagram(now, data);
        let claim = match (client.relayed(), relayed) {
            (Some(relay), Some((peer, data))) => Claim::Relayed { relay, peer, data },
            _ => Claim::Consumed,
        };
        self.pump(now);
        Some(claim)
    }

    /// Wraps a connection's datagram that leaves from relay candidate `relay`; false if no
    /// allocation holds it any more.
    pub fn send(&mut self, relay: SocketAddr, to: SocketAddr, data: &[u8], now: Instant) -> bool {
        let Some(client) = self.client_at(relay) else { return false };
        client.send_to(to, data, now);
        self.pump(now);
        true
    }

    /// Lets `peer` reach every allocation (TURN permissions are per IP).
    pub fn permit(&mut self, peer: SocketAddr, now: Instant) {
        for client in self.relays.iter_mut().filter_map(|r| r.client.as_mut()) {
            client.create_permission(peer, now);
        }
        self.pump(now);
    }

    pub fn forget(&mut self, peer: SocketAddr, now: Instant) {
        for client in self.relays.iter_mut().filter_map(|r| r.client.as_mut()) {
            client.forget(peer, now);
        }
    }

    pub fn poll_transmit(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        self.transmits.pop_front()
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        let stun = self.stun.iter().flat_map(|s| s.request.as_ref().map(|(_, _, r)| r.deadline()).or(s.next_at));
        let turn = self.relays.iter().flat_map(|r| r.client.as_ref().and_then(TurnClient::poll_timeout).or(r.retry_at));
        stun.chain(turn).min()
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        for stun in &mut self.stun {
            if let Some((_, bytes, timer)) = stun.request.as_mut() {
                match timer.poll(now) {
                    Step::Wait => {}
                    Step::Resend => self.transmits.push_back((stun.server, bytes.clone())),
                    Step::GiveUp => {
                        tracing::debug!(server = %stun.server, "STUN server did not answer");
                        (stun.request, stun.mapped, stun.next_at) = (None, None, Some(now + STUN_RETRY));
                    }
                }
            } else if stun.next_at.is_some_and(|at| at <= now) {
                let (tid, bytes) = turn::stun_binding_request();
                self.transmits.push_back((stun.server, bytes.clone()));
                (stun.request, stun.next_at) = (Some((tid, bytes, Retransmit::start(now))), None);
            }
        }
        for relay in &mut self.relays {
            if relay.retry_at.is_some_and(|at| at <= now) {
                let TurnServer { addr, username, password } = &relay.server;
                (relay.client, relay.retry_at) = (Some(TurnClient::new(*addr, username.as_str(), password.as_str(), now)), None);
            } else if let Some(client) = relay.client.as_mut().filter(|c| c.poll_timeout().is_some_and(|at| at <= now)) {
                client.handle_timeout(now);
            }
        }
        self.pump(now);
    }

    fn client_at(&mut self, relay: SocketAddr) -> Option<&mut TurnClient> {
        self.relays.iter_mut().filter_map(|r| r.client.as_mut()).find(|c| c.relayed() == Some(relay))
    }

    /// Moves the TURN clients' output to the queue and schedules retries for lost allocations.
    fn pump(&mut self, now: Instant) {
        for relay in &mut self.relays {
            let Some(client) = relay.client.as_mut() else { continue };
            while let Some(bytes) = client.poll_transmit() {
                self.transmits.push_back((client.server(), bytes));
            }
            while let Some(event) = client.poll_event() {
                match event {
                    TurnEvent::Allocated { relayed, mapped } => {
                        tracing::debug!(server = %relay.server.addr, %relayed, "TURN allocation");
                        relay.mapped = mapped;
                    }
                    TurnEvent::Failed(reason) => tracing::debug!(server = %relay.server.addr, "TURN allocation failed: {reason}"),
                }
            }
            if client.is_done() {
                (relay.client, relay.mapped, relay.retry_at) = (None, None, Some(now + TURN_RETRY));
            }
        }
    }
}
