use std::net::SocketAddr;
use std::time::Instant;

use p384::ecdsa::SigningKey;
use str0m::change::SdpOffer;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, IceCreds, Input, Rtc};

use super::Connection;
use crate::identity::identity_line;
use crate::server_identity::{server_identity, unix_now};
use crate::{cert, sdp, Error};

impl Connection {
    /// Answers a direct-connect offer (`POST /v1/join`) the way BDS does: one host candidate at
    /// `local`, the DTLS client role, and an answer signed by `key` under domain `self`. The
    /// client's own assertion is not checked here: see [`crate::verify_offer`].
    pub fn answer(offer: &str, local: SocketAddr, key: &SigningKey, now: Instant) -> Result<(Self, String), Error> {
        Self::new_answer(offer, Some(local), key, now)
    }

    /// Answers a `CONNECTREQUEST` without candidates; they follow through
    /// [`Connection::add_local_candidate`] as `CANDIDATEADD`s.
    pub fn trickle_answer(offer: &str, key: &SigningKey, now: Instant) -> Result<(Self, String), Error> {
        Self::new_answer(offer, None, key, now)
    }

    fn new_answer(offer: &str, embedded: Option<SocketAddr>, key: &SigningKey, now: Instant) -> Result<(Self, String), Error> {
        let (ufrag, pass) = sdp::ice_credentials();
        let config = Rtc::builder()
            .set_local_ice_credentials(IceCreds { ufrag: ufrag.clone(), pass })
            .set_dtls_cert(cert::libwebrtc_certificate()?);
        let mut rtc = Box::new(config.build(now));
        if let Some(local) = embedded {
            rtc.add_local_candidate(Candidate::host(local, "udp").map_err(|e| Error::Sdp(e.to_string()))?);
        }
        let parsed = SdpOffer::from_sdp_string(&sdp::offer_for_str0m(offer)).map_err(|e| Error::Sdp(e.to_string()))?;
        let str0m_answer = rtc.sdp_api().accept_offer(parsed)?.to_sdp_string();
        let identity = server_identity(key, unix_now());
        let answer = sdp::vanilla_sdp(&str0m_answer, &identity_line(&str0m_answer, &identity), embedded);
        let mut conn = Self::new(rtc, embedded, ufrag, sdp::str0m_mid(&str0m_answer));
        if let Some(size) = sdp::max_message_size(offer)? {
            conn.max_message = size;
        }
        conn.drive(now);
        Ok((conn, answer))
    }

    /// Our ICE username fragment: what the peer's binding requests name us by.
    pub fn local_ufrag(&self) -> &str {
        &self.ufrag
    }

    /// Whether a datagram for the host candidate is this connection's: one from an address ICE
    /// already uses, or a STUN message carrying our credentials. For routing on a shared socket.
    pub fn accepts(&self, now: Instant, source: SocketAddr, data: &[u8]) -> bool {
        let Some(host) = self.host else { return false };
        Receive::new(Protocol::Udp, source, host, data).is_ok_and(|receive| self.rtc.accepts(&Input::Receive(now, receive)))
    }

    /// Bytes given to [`Connection::send`] that the reliable channel has not taken yet.
    pub fn queued_bytes(&self) -> usize {
        self.unsent.iter().map(|m| m.len()).sum::<usize>() + self.outbound.iter().map(Vec::len).sum::<usize>()
    }
}
