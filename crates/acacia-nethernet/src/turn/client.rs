use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::requests::{Pending, Request};
use super::stun::{self, Attr, Class, Message, Method, TransactionId};
use super::Step;

/// libwebrtc refreshes permissions and channel binds 1 min before the 5 min permission expiry.
pub(super) const PEER_REFRESH: Duration = Duration::from_secs(4 * 60);
const FIRST_CHANNEL: u16 = 0x4000;
const LAST_CHANNEL: u16 = 0x4FFF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEvent {
    /// `relayed` becomes a relay candidate, `mapped` (when the server sent it) a srflx candidate.
    Allocated { relayed: SocketAddr, mapped: Option<SocketAddr> },
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Allocating,
    Allocated { relayed: SocketAddr, refresh_at: Option<Instant> },
    Closing,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bind {
    Unbound,
    Binding,
    Bound,
}

/// Per-peer state like libwebrtc's `TurnEntry`: a permission, and a channel bound on first send.
pub(super) struct Entry {
    pub channel: u16,
    pub bind: Bind,
    pub permission_at: Option<Instant>,
    pub bind_at: Option<Instant>,
}

/// One allocation on one TURN server over UDP. Sans-IO: every datagram from
/// [`TurnClient::poll_transmit`] goes to [`TurnClient::server`], and everything that server sends
/// goes to [`TurnClient::handle_datagram`].
pub struct TurnClient {
    pub(super) server: SocketAddr,
    pub(super) redirects: u8,
    pub(super) username: String,
    pub(super) password: String,
    pub(super) realm: Option<String>,
    pub(super) nonce: Option<String>,
    pub(super) key: Option<[u8; 16]>,
    pub(super) state: State,
    pub(super) pending: HashMap<TransactionId, Pending>,
    pub(super) entries: HashMap<SocketAddr, Entry>,
    pub(super) channels: HashMap<u16, SocketAddr>,
    next_channel: u16,
    pub(super) transmits: VecDeque<Vec<u8>>,
    pub(super) events: VecDeque<TurnEvent>,
}

impl TurnClient {
    /// Starts allocating; the first Allocate is unauthenticated and expects a 401 challenge.
    pub fn new(server: SocketAddr, username: impl Into<String>, password: impl Into<String>, now: Instant) -> Self {
        let mut client = Self {
            server,
            redirects: 0,
            username: username.into(),
            password: password.into(),
            realm: None,
            nonce: None,
            key: None,
            state: State::Allocating,
            pending: HashMap::new(),
            entries: HashMap::new(),
            channels: HashMap::new(),
            next_channel: FIRST_CHANNEL,
            transmits: VecDeque::new(),
            events: VecDeque::new(),
        };
        client.start(Request::Allocate, now, 0);
        client
    }

    pub fn server(&self) -> SocketAddr {
        self.server
    }

    pub fn relayed(&self) -> Option<SocketAddr> {
        match self.state {
            State::Allocated { relayed, .. } => Some(relayed),
            _ => None,
        }
    }

    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    /// Installs a permission for `peer` (a remote candidate) so its datagrams get relayed to us.
    pub fn create_permission(&mut self, peer: SocketAddr, now: Instant) {
        self.entry(peer, now);
    }

    /// Wraps `data` for `peer`: a Send indication until the channel is bound, ChannelData after.
    pub fn send_to(&mut self, peer: SocketAddr, data: &[u8], now: Instant) {
        if self.relayed().is_none() {
            return tracing::trace!(%peer, "TURN send before allocation; dropped");
        }
        let Some((channel, bind)) = self.entry(peer, now) else { return };
        if bind == Bind::Bound {
            return self.transmits.push_back(stun::channel_data(channel, data));
        }
        let indication = Message::new(Class::Indication, Method::Send, stun::transaction_id())
            .with(Attr::XorPeerAddress(peer))
            .with(Attr::Data(data.to_vec()));
        self.transmits.push_back(indication.encode(None, false));
        if bind == Bind::Unbound {
            self.entries.get_mut(&peer).expect("entry just created").bind = Bind::Binding;
            self.start(Request::ChannelBind(peer), now, 0);
        }
    }

    /// Consumes control responses; returns relayed peer data (Data indication or ChannelData).
    pub fn handle_datagram(&mut self, now: Instant, data: &[u8]) -> Option<(SocketAddr, Vec<u8>)> {
        if self.state == State::Done {
            return None;
        }
        if !stun::is_stun(data) {
            let (channel, payload) = stun::parse_channel_data(data)?;
            return self.channels.get(&channel).map(|peer| (*peer, payload.to_vec()));
        }
        let msg = Message::decode(data).map_err(|e| tracing::trace!("TURN: dropping datagram: {e}")).ok()?;
        match msg.class {
            Class::Indication if msg.method == Method::Data => {
                let peer = msg.xor_peer()?;
                Some((peer, msg.into_data()?))
            }
            Class::Success | Class::Error => {
                self.handle_response(now, msg, data);
                None
            }
            _ => None,
        }
    }

    pub fn poll_transmit(&mut self) -> Option<Vec<u8>> {
        self.transmits.pop_front()
    }

    pub fn poll_event(&mut self) -> Option<TurnEvent> {
        self.events.pop_front()
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        let refresh = match self.state {
            State::Allocated { refresh_at, .. } => refresh_at,
            _ => None,
        };
        let peers = self.entries.values().flat_map(|e| [e.permission_at, e.bind_at]).flatten();
        let retransmits = self.pending.values().map(|p| p.timer.deadline());
        refresh.into_iter().chain(peers).chain(retransmits).min()
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        let mut expired = Vec::new();
        for (tid, p) in &mut self.pending {
            match p.timer.poll(now) {
                Step::Wait => {}
                Step::Resend => self.transmits.push_back(p.bytes.clone()),
                Step::GiveUp => expired.push(*tid),
            }
        }
        for tid in expired {
            let p = self.pending.remove(&tid).expect("expired transaction is pending");
            self.on_failure(p.request, "timed out");
        }
        if let State::Allocated { refresh_at: Some(at), relayed } = self.state
            && at <= now
        {
            self.state = State::Allocated { relayed, refresh_at: None };
            self.start(Request::Refresh { lifetime: None }, now, 0);
        }
        let mut due = Vec::new();
        for (peer, e) in &mut self.entries {
            if e.permission_at.take_if(|at| *at <= now).is_some() {
                due.push(Request::Permission(*peer));
            }
            if e.bind_at.take_if(|at| *at <= now).is_some() {
                due.push(Request::ChannelBind(*peer));
            }
        }
        for request in due {
            self.start(request, now, 0);
        }
    }

    /// Releases the allocation (Refresh with LIFETIME 0), dropping every other request.
    pub fn close(&mut self, now: Instant) {
        self.pending.clear();
        self.entries.clear();
        self.channels.clear();
        if self.relayed().is_some() {
            self.state = State::Closing;
            self.start(Request::Refresh { lifetime: Some(0) }, now, 0);
        } else {
            self.state = State::Done;
        }
    }

    /// The peer's channel and bind state, creating the entry (and its permission) on first use.
    fn entry(&mut self, peer: SocketAddr, now: Instant) -> Option<(u16, Bind)> {
        if let Some(e) = self.entries.get(&peer) {
            return Some((e.channel, e.bind));
        }
        if self.next_channel > LAST_CHANNEL {
            tracing::debug!(%peer, "TURN channel numbers exhausted");
            return None;
        }
        let channel = self.next_channel;
        self.next_channel += 1;
        self.entries.insert(peer, Entry { channel, bind: Bind::Unbound, permission_at: None, bind_at: None });
        self.channels.insert(channel, peer);
        if self.relayed().is_some() {
            self.start(Request::Permission(peer), now, 0);
        }
        Some((channel, Bind::Unbound))
    }
}
