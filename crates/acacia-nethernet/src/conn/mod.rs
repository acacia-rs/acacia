mod candidates;
mod pump;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::Instant;

use bytes::Bytes;
use str0m::change::{SdpAnswer, SdpPendingOffer};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, IceCreds, Input, Rtc};

pub use candidates::LocalCandidate;

use crate::frame::Reassembler;
use crate::identity::identity_line;
use crate::server_identity::verify_answer;
use crate::{cert, sdp, Error, Identity};

/// Peers that omit `a=max-message-size` accept 64 KiB (RFC 8841).
const DEFAULT_MAX_MESSAGE: usize = 65536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Both data channels are open; game messages flow from here on.
    Open,
    Closed(String),
}

/// A datagram to send. `source` is the local candidate it leaves from: the host candidate, or a
/// relayed address whose TURN allocation must wrap it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transmit {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub contents: Vec<u8>,
}

/// One NetherNet connection as the offering client. Sans-IO: feed it datagrams and time, send what
/// [`Connection::poll_transmit`] yields. Datagrams for the host candidate go to
/// [`Connection::handle_datagram`] (for a SOCKS5 relay, the advertised address stands in for the
/// socket); ones unwrapped from a TURN allocation go to [`Connection::handle_relayed`].
pub struct Connection {
    /// Boxed: `Rtc` is large enough to overflow small (debug, task) stacks when moved around.
    rtc: Box<Rtc>,
    host: Option<SocketAddr>,
    relays: Vec<SocketAddr>,
    ufrag: String,
    /// str0m's mid for the data section; the wire uses the vanilla `0`.
    mid: String,
    pending_offer: Option<SdpPendingOffer>,
    /// Trickled remote candidates that arrived before the answer.
    early_candidates: Vec<Candidate>,
    server_key: Option<p384::ecdsa::VerifyingKey>,
    /// Game-hosted (LAN) worlds answer without `a=identity`; BDS always asserts one.
    identityless_host_ok: bool,
    reliable: ChannelId,
    unreliable: ChannelId,
    open: [bool; 2],
    closed: bool,
    max_message: usize,
    reassembler: Reassembler,
    /// Framed fragments waiting for the reliable channel to open or drain.
    outbound: VecDeque<Vec<u8>>,
    unsent: VecDeque<Bytes>,
    transmits: VecDeque<Transmit>,
    messages: VecDeque<Bytes>,
    events: VecDeque<Event>,
    timeout: Option<Instant>,
}

impl Connection {
    /// Builds the SDP offer (identity asserted) with `local` embedded, for `POST /v1/join/{id}`.
    pub fn offer(local: SocketAddr, identity: &Identity, now: Instant) -> Result<(Self, String), Error> {
        Self::new_offer(identity, now, Some(local))
    }

    /// Builds a candidate-less offer for `CONNECTREQUEST`; candidates follow through
    /// [`Connection::add_local_candidate`] as `CANDIDATEADD`s.
    pub fn trickle_offer(identity: &Identity, now: Instant) -> Result<(Self, String), Error> {
        Self::new_offer(identity, now, None)
    }

    fn new_offer(identity: &Identity, now: Instant, embedded: Option<SocketAddr>) -> Result<(Self, String), Error> {
        let (ufrag, pass) = sdp::ice_credentials();
        let config = Rtc::builder()
            .set_local_ice_credentials(IceCreds { ufrag: ufrag.clone(), pass })
            .set_dtls_cert(cert::libwebrtc_certificate()?);
        let mut rtc = Box::new(config.build(now));
        if let Some(local) = embedded {
            rtc.add_local_candidate(Candidate::host(local, "udp").map_err(|e| Error::Sdp(e.to_string()))?);
        }
        let mut api = rtc.sdp_api();
        let reliable = api.add_channel_with_config(ChannelConfig {
            label: "ReliableDataChannel".into(),
            ordered: true,
            reliability: Reliability::Reliable,
            ..Default::default()
        });
        let unreliable = api.add_channel_with_config(ChannelConfig {
            label: "UnreliableDataChannel".into(),
            ordered: false,
            reliability: Reliability::MaxRetransmits { retransmits: 0 },
            ..Default::default()
        });
        let (offer, pending) = api.apply().ok_or_else(|| Error::Sdp("no changes to offer".into()))?;
        let str0m_offer = offer.to_sdp_string();
        let offer = sdp::vanilla_offer(&str0m_offer, &identity_line(&str0m_offer, identity), embedded);
        let conn = Self {
            rtc,
            host: embedded,
            relays: Vec::new(),
            ufrag,
            mid: sdp::str0m_mid(&str0m_offer),
            pending_offer: Some(pending),
            early_candidates: Vec::new(),
            server_key: None,
            identityless_host_ok: false,
            reliable,
            unreliable,
            open: [false; 2],
            closed: false,
            max_message: DEFAULT_MAX_MESSAGE,
            reassembler: Reassembler::default(),
            outbound: VecDeque::new(),
            unsent: VecDeque::new(),
            transmits: VecDeque::new(),
            messages: VecDeque::new(),
            events: VecDeque::new(),
            timeout: None,
        };
        Ok((conn, offer))
    }

    /// Accepts an answer without `a=identity` (a game-hosted LAN world); one that has it is still verified.
    pub fn allow_identityless_host(&mut self) {
        self.identityless_host_ok = true;
    }

    pub fn host_candidate(&self) -> Option<SocketAddr> {
        self.host
    }

    /// The operator key the server proved it holds (`cpk`), once the answer is accepted. Pin it to
    /// detect a changed server identity (the vanilla client's trust-on-first-use).
    pub fn server_key(&self) -> Option<&p384::ecdsa::VerifyingKey> {
        self.server_key.as_ref()
    }

    pub fn accept_answer(&mut self, answer: &str, now: Instant) -> Result<(), Error> {
        let pending = self.pending_offer.take().ok_or_else(|| Error::Sdp("answer already accepted".into()))?;
        if let Some(size) = answer.lines().find_map(|l| l.strip_prefix("a=max-message-size:")) {
            self.max_message = size.trim().parse().map_err(|_| Error::Sdp("bad max-message-size".into()))?;
        }
        let asserted = answer.lines().any(|l| l.starts_with(crate::identity::IDENTITY));
        if asserted || !self.identityless_host_ok {
            let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            self.server_key = Some(verify_answer(answer, unix)?);
        }
        let answer = SdpAnswer::from_sdp_string(&sdp::answer_for_str0m(answer, &self.mid)).map_err(|e| Error::Sdp(e.to_string()))?;
        self.rtc.sdp_api().accept_answer(pending, answer)?;
        for c in std::mem::take(&mut self.early_candidates) {
            self.rtc.add_remote_candidate(c);
        }
        self.drive(now);
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.open == [true; 2]
    }

    /// Queues one game message (sent once the channels are open).
    pub fn send(&mut self, msg: Bytes, now: Instant) {
        self.unsent.push_back(msg);
        self.drive(now);
    }

    /// A datagram that arrived at the host candidate.
    pub fn handle_datagram(&mut self, now: Instant, source: SocketAddr, data: &[u8]) {
        match self.host {
            Some(host) => self.receive(now, host, source, data),
            None => tracing::trace!(%source, "dropping datagram: no host candidate"),
        }
    }

    /// A datagram `peer` sent to our TURN allocation at `relay`, already unwrapped.
    pub fn handle_relayed(&mut self, now: Instant, relay: SocketAddr, peer: SocketAddr, data: &[u8]) {
        if self.relays.contains(&relay) {
            self.receive(now, relay, peer, data);
        } else {
            tracing::trace!(%relay, "dropping datagram: unknown relay");
        }
    }

    fn receive(&mut self, now: Instant, destination: SocketAddr, source: SocketAddr, data: &[u8]) {
        match Receive::new(Protocol::Udp, source, destination, data) {
            Ok(receive) => {
                if let Err(e) = self.rtc.handle_input(Input::Receive(now, receive)) {
                    return self.close_with(format!("input: {e}"));
                }
            }
            Err(e) => tracing::trace!(%source, "dropping datagram: {e}"),
        }
        self.drive(now);
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        if self.timeout.is_some_and(|t| t <= now) {
            if let Err(e) = self.rtc.handle_input(Input::Timeout(now)) {
                return self.close_with(format!("timeout: {e}"));
            }
            self.drive(now);
        }
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        self.timeout
    }

    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.transmits.pop_front()
    }

    /// Next reassembled game message.
    pub fn poll_message(&mut self) -> Option<Bytes> {
        self.messages.pop_front()
    }

    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Flushes queued messages, then starts SCTP shutdown and DTLS close_notify; the resulting
    /// datagrams are left in [`Connection::poll_transmit`].
    pub fn close(&mut self, now: Instant) {
        self.drive(now);
        if let Err(e) = self.rtc.close() {
            tracing::debug!("close: {e}");
        }
        self.drive(now);
        self.close_with("local close".into());
    }
}
