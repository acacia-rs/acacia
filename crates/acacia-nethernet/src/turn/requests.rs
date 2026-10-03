//! TurnClient's requests: building them, retrying auth challenges, applying responses.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::client::{Bind, State, TurnClient, TurnEvent, PEER_REFRESH};
use super::stun::{self, Attr, Class, Message, Method};
use super::Retransmit;

const UDP: u8 = 17;
const DEFAULT_LIFETIME: u32 = 600;

/// libwebrtc gives up after redirecting through a few servers (`TurnPort` attempted-server set).
const MAX_REDIRECTS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Request {
    Allocate,
    Refresh { lifetime: Option<u32> },
    Permission(SocketAddr),
    ChannelBind(SocketAddr),
}

pub(super) struct Pending {
    pub request: Request,
    pub bytes: Vec<u8>,
    pub timer: Retransmit,
    auth_retries: u8,
}

impl TurnClient {
    pub(super) fn start(&mut self, request: Request, now: Instant, auth_retries: u8) {
        let tid = stun::transaction_id();
        let (method, mut attrs) = match request {
            Request::Allocate => (Method::Allocate, vec![Attr::RequestedTransport(UDP)]),
            Request::Refresh { lifetime } => (Method::Refresh, lifetime.map(Attr::Lifetime).into_iter().collect()),
            Request::Permission(peer) => (Method::CreatePermission, vec![Attr::XorPeerAddress(peer)]),
            Request::ChannelBind(peer) => {
                let channel = self.entries.get(&peer).map_or(0, |e| e.channel);
                (Method::ChannelBind, vec![Attr::ChannelNumber(channel), Attr::XorPeerAddress(peer)])
            }
        };
        if let (Some(realm), Some(nonce)) = (&self.realm, &self.nonce) {
            attrs.extend([Attr::Username(self.username.clone()), Attr::Realm(realm.clone()), Attr::Nonce(nonce.clone())]);
        }
        let msg = Message { class: Class::Request, method, transaction_id: tid, attrs };
        let bytes = msg.encode(self.key.as_ref().map(|k| &k[..]), false);
        self.transmits.push_back(bytes.clone());
        self.pending.insert(tid, Pending { request, bytes, timer: Retransmit::start(now), auth_retries });
    }

    pub(super) fn handle_response(&mut self, now: Instant, msg: Message, raw: &[u8]) {
        let Some(p) = self.pending.remove(&msg.transaction_id) else { return };
        if msg.class == Class::Success && self.key.is_some_and(|k| !stun::verify_integrity(raw, &k)) {
            tracing::debug!("TURN: response failed MESSAGE-INTEGRITY; ignored");
            self.pending.insert(msg.transaction_id, p);
            return;
        }
        if msg.class == Class::Success {
            return self.on_success(now, p.request, &msg);
        }
        let (code, reason) = msg.error_code().unwrap_or((0, "no ERROR-CODE"));
        // Azure TURN redirects allocations; the server may change only before we allocate.
        if code == 300
            && matches!(p.request, Request::Allocate)
            && self.redirects < MAX_REDIRECTS
            && let Some(alternate) = msg.alternate_server()
        {
            tracing::debug!(from = %self.server, to = %alternate, "TURN: allocation redirected");
            self.redirects += 1;
            self.server = alternate;
            (self.realm, self.nonce, self.key) = (None, None, None);
            return self.start(Request::Allocate, now, 0);
        }
        // 401 gets one answer to its challenge (libwebrtc fails a second); 438 just renews the nonce.
        let retry = match code {
            401 => p.auth_retries == 0,
            438 => p.auth_retries < 2,
            _ => false,
        };
        if retry && let Some(nonce) = msg.nonce() {
            self.realm = msg.realm().or(self.realm.take());
            self.nonce = Some(nonce);
            self.key = self.realm.as_ref().map(|r| stun::long_term_key(&self.username, r, &self.password));
            return self.start(p.request, now, p.auth_retries + 1);
        }
        self.on_failure(p.request, &format!("error {code} {reason}"));
    }

    fn on_success(&mut self, now: Instant, request: Request, msg: &Message) {
        let lifetime = msg.lifetime().unwrap_or(DEFAULT_LIFETIME);
        match request {
            Request::Allocate => {
                let Some(relayed) = msg.xor_relayed() else {
                    return self.fail("Allocate success without XOR-RELAYED-ADDRESS".into());
                };
                self.state = State::Allocated { relayed, refresh_at: Some(now + refresh_delay(lifetime)) };
                self.events.push_back(TurnEvent::Allocated { relayed, mapped: msg.xor_mapped() });
                let peers: Vec<_> = self.entries.keys().copied().collect();
                for peer in peers {
                    self.start(Request::Permission(peer), now, 0);
                }
            }
            Request::Refresh { lifetime: Some(0) } => self.state = State::Done,
            Request::Refresh { .. } => {
                if let State::Allocated { relayed, .. } = self.state {
                    self.state = State::Allocated { relayed, refresh_at: Some(now + refresh_delay(lifetime)) };
                }
            }
            Request::Permission(peer) => {
                if let Some(e) = self.entries.get_mut(&peer) {
                    e.permission_at = Some(now + PEER_REFRESH);
                }
            }
            Request::ChannelBind(peer) => {
                if let Some(e) = self.entries.get_mut(&peer) {
                    e.bind = Bind::Bound;
                    e.bind_at = Some(now + PEER_REFRESH);
                }
            }
        }
    }

    pub(super) fn on_failure(&mut self, request: Request, why: &str) {
        match request {
            Request::Allocate => self.fail(format!("Allocate: {why}")),
            Request::Refresh { lifetime: Some(0) } => self.state = State::Done,
            Request::Refresh { .. } => self.fail(format!("Refresh: {why}")),
            Request::Permission(peer) | Request::ChannelBind(peer) => {
                tracing::debug!(%peer, ?request, "TURN peer request failed: {why}");
                if let Some(e) = self.entries.remove(&peer) {
                    self.channels.remove(&e.channel);
                }
            }
        }
    }

    fn fail(&mut self, reason: String) {
        self.state = State::Done;
        self.pending.clear();
        self.events.push_back(TurnEvent::Failed(reason));
    }
}

/// libwebrtc `TurnPort::ScheduleRefresh`: a minute early, or halfway for lifetimes under 2 min.
fn refresh_delay(lifetime: u32) -> Duration {
    Duration::from_secs(match lifetime {
        0..120 => return Duration::from_millis(u64::from(lifetime) * 500),
        3601.. => 3600 - 60,
        _ => u64::from(lifetime) - 60,
    })
}
