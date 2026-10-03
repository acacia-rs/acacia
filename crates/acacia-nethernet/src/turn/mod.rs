//! Network-free STUN/TURN client for srflx and relay ICE candidates (str0m has neither). The driver
//! owns the UDP socket: it sends what these types yield to the STUN/TURN server and feeds back what
//! that server sends. Wire behaviour follows libwebrtc (p2p/base/turn_port.cc), which vanilla uses.

mod attr;
mod client;
#[cfg(test)]
mod client_tests;
mod credentials;
mod requests;
pub mod stun;
#[cfg(test)]
mod stun_tests;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub use client::{TurnClient, TurnEvent};
pub use credentials::{IceServer, IceServers, IceUrl, Scheme, Transport};
use stun::{Class, Message, Method, TransactionId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TurnError {
    #[error("STUN: {0}")]
    Stun(&'static str),
    #[error("ICE server config: {0}")]
    Config(String),
}

/// libwebrtc's request pacing (stun_request.cc): resend after 250 ms, doubling to an 8 s cap, 9 sends.
const INITIAL_RTO: Duration = Duration::from_millis(250);
const MAX_RTO: Duration = Duration::from_secs(8);
const MAX_SENDS: u32 = 9;

/// Retransmission schedule of one STUN request, counting the send made when it starts.
#[derive(Debug, Clone)]
pub struct Retransmit {
    sends: u32,
    next: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Wait,
    Resend,
    GiveUp,
}

impl Retransmit {
    pub fn start(now: Instant) -> Self {
        Self { sends: 1, next: now + INITIAL_RTO }
    }

    pub fn deadline(&self) -> Instant {
        self.next
    }

    pub fn poll(&mut self, now: Instant) -> Step {
        if now < self.next {
            return Step::Wait;
        }
        if self.sends >= MAX_SENDS {
            return Step::GiveUp;
        }
        self.next = now + (INITIAL_RTO * 2u32.pow(self.sends)).min(MAX_RTO);
        self.sends += 1;
        Step::Resend
    }
}

/// A Binding request for srflx discovery; bare like libwebrtc's (stun_port.cc `StunBindingRequest`).
pub fn stun_binding_request() -> (TransactionId, Vec<u8>) {
    let tid = stun::transaction_id();
    (tid, Message::new(Class::Request, Method::Binding, tid).encode(None, false))
}

/// The server-reflexive address in a Binding success answering `tid`.
pub fn parse_binding_response(tid: &TransactionId, data: &[u8]) -> Option<SocketAddr> {
    let msg = Message::decode(data).ok()?;
    let ours = msg.class == Class::Success && msg.method == Method::Binding && msg.transaction_id == *tid;
    ours.then(|| msg.mapped()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retransmit_follows_libwebrtc_schedule() {
        let t0 = Instant::now();
        let mut r = Retransmit::start(t0);
        let mut at = t0;
        let mut gaps = vec![];
        loop {
            let gap = r.deadline() - at;
            at = r.deadline();
            assert_eq!(r.poll(at - Duration::from_millis(1)), Step::Wait);
            gaps.push(gap.as_millis());
            if r.poll(at) == Step::GiveUp {
                break;
            }
        }
        assert_eq!(gaps, [250, 500, 1000, 2000, 4000, 8000, 8000, 8000, 8000]);
    }

    #[test]
    fn binding_roundtrip() {
        let (tid, req) = stun_binding_request();
        assert_eq!(req.len(), 20);
        let addr: SocketAddr = "203.0.113.7:50000".parse().unwrap();
        let resp = Message::new(Class::Success, Method::Binding, tid).with(stun::Attr::XorMappedAddress(addr));
        assert_eq!(parse_binding_response(&tid, &resp.encode(None, true)), Some(addr));
        assert_eq!(parse_binding_response(&[0; 12], &resp.encode(None, false)), None);
    }
}
