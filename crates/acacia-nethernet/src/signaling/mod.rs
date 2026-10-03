//! Sans-IO client for the Minecraft signaling service (Realms, friends' worlds). The caller owns the
//! WebSocket: it passes received text frames in and sends what [`SignalingSession::poll_text`] yields.
//! Spec: docs/research/nethernet-signaling.md §2.

mod codec;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::turn::IceServers;
use crate::Signal;
use codec::Inbound;

/// The flavour the realm's `networkProtocol` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingProtocol {
    /// `NETHERNET`: `/ws/v1.0/signaling/<own id>`; the server pushes TURN credentials.
    Legacy,
    /// `NETHERNET_JSONRPC`: `/ws/v1.0/messaging/connect`; credentials come from `Signaling_TurnAuth_v1_0`.
    JsonRpc,
}

impl SignalingProtocol {
    /// The WebSocket URL on `host` (e.g. `signal-<region>.franchise.minecraft-services.net`).
    pub fn url(self, host: &str, own_network_id: u64) -> String {
        match self {
            Self::Legacy => format!("wss://{host}/ws/v1.0/signaling/{own_network_id}"),
            Self::JsonRpc => format!("wss://{host}/ws/v1.0/messaging/connect"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SignalingEvent {
    /// TURN/STUN servers for this session's connections.
    Credentials(IceServers),
    /// A signal from peer `from` (a NetherNet id or PMID, as the service names it).
    Signal { from: String, signal: Signal },
    /// The service reported a failure; the WebSocket may stay open.
    Error(String),
}

/// `pingFrequency` (00:00:48) from `signaling-afd…/api/v1.0/configuration`, which vanilla fetches.
pub const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(48);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub struct SignalingSession {
    protocol: SignalingProtocol,
    own_network_id: u64,
    ping_interval: Duration,
    next_ping: Instant,
    /// JSON-RPC request id → (method, deadline).
    pending: HashMap<String, (&'static str, Instant)>,
    out: VecDeque<String>,
    events: VecDeque<SignalingEvent>,
}

impl SignalingSession {
    /// Call once the WebSocket is open. JSON-RPC sessions request TURN credentials right away.
    pub fn new(protocol: SignalingProtocol, own_network_id: u64, ping_interval: Duration, now: Instant) -> Self {
        let mut s = Self {
            protocol,
            own_network_id,
            ping_interval,
            next_ping: now + ping_interval,
            pending: HashMap::new(),
            out: VecDeque::new(),
            events: VecDeque::new(),
        };
        if protocol == SignalingProtocol::JsonRpc {
            s.request(codec::METHOD_TURN_AUTH, json!({}), now);
        }
        s
    }

    pub fn own_network_id(&self) -> u64 {
        self.own_network_id
    }

    pub fn send_signal(&mut self, to: &str, signal: &Signal, now: Instant) {
        match self.protocol {
            SignalingProtocol::Legacy => self.out.push_back(codec::legacy_signal(to, signal)),
            SignalingProtocol::JsonRpc => {
                let id = codec::uuid();
                self.out.push_back(codec::rpc_signal(&id, to, self.own_network_id, signal));
                self.pending.insert(id, (codec::METHOD_SEND, now + REQUEST_TIMEOUT));
            }
        }
    }

    pub fn handle_text(&mut self, text: &str) {
        for inbound in codec::decode(text) {
            match inbound {
                Inbound::Signal { from, signal } => self.events.push_back(SignalingEvent::Signal { from, signal }),
                Inbound::Credentials(c) => self.credentials(&c),
                Inbound::ServerError(e) => self.events.push_back(SignalingEvent::Error(e)),
                Inbound::Ack(id) => self.out.push_back(codec::rpc_ack(&id)),
                Inbound::Response { id, result } => self.handle_response(&id, result),
                Inbound::Ignored => tracing::trace!(%text, "signaling frame ignored"),
            }
        }
    }

    fn handle_response(&mut self, id: &str, result: Result<Value, String>) {
        let Some((method, _)) = self.pending.remove(id) else { return };
        match (method, result) {
            (codec::METHOD_TURN_AUTH, Ok(creds)) => self.credentials(&creds),
            (_, Ok(_)) => {}
            (method, Err(e)) => self.events.push_back(SignalingEvent::Error(format!("{method}: {e}"))),
        }
    }

    fn credentials(&mut self, json: &Value) {
        self.events.push_back(match IceServers::from_json(json) {
            Ok(servers) => SignalingEvent::Credentials(servers),
            Err(e) => SignalingEvent::Error(e.to_string()),
        });
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        if now >= self.next_ping {
            self.next_ping = now + self.ping_interval;
            match self.protocol {
                SignalingProtocol::Legacy => self.out.push_back(codec::legacy_ping()),
                SignalingProtocol::JsonRpc => self.out.push_back(codec::rpc_request(&codec::uuid(), codec::METHOD_PING, json!([]))),
            }
        }
        let expired: Vec<String> = self.pending.iter().filter(|(_, (_, d))| *d <= now).map(|(id, _)| id.clone()).collect();
        for id in expired {
            if let Some((method, _)) = self.pending.remove(&id) {
                self.events.push_back(SignalingEvent::Error(format!("{method} timed out")));
            }
        }
    }

    pub fn poll_timeout(&self) -> Instant {
        self.pending.values().map(|(_, d)| *d).fold(self.next_ping, Instant::min)
    }

    pub fn poll_text(&mut self) -> Option<String> {
        self.out.pop_front()
    }

    pub fn poll_event(&mut self) -> Option<SignalingEvent> {
        self.events.pop_front()
    }

    fn request(&mut self, method: &'static str, params: Value, now: Instant) {
        let id = codec::uuid();
        self.out.push_back(codec::rpc_request(&id, method, params));
        self.pending.insert(id, (method, now + REQUEST_TIMEOUT));
    }
}
