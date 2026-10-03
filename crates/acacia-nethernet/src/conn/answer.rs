use std::net::SocketAddr;
use std::time::Instant;

use p384::ecdsa::SigningKey;
use str0m::change::SdpOffer;
use str0m::{Candidate, IceCreds, Rtc};

use super::Connection;
use crate::identity::identity_line;
use crate::server_identity::{server_identity, unix_now};
use crate::{cert, sdp, Error};

impl Connection {
    /// Answers a direct-connect offer (`POST /v1/join`) the way BDS does: one host candidate at
    /// `local`, the DTLS client role, and an answer signed by `key` under domain `self`. The
    /// client's own assertion is not checked, so this host admits anyone.
    pub fn answer(offer: &str, local: SocketAddr, key: &SigningKey, now: Instant) -> Result<(Self, String), Error> {
        let (ufrag, pass) = sdp::ice_credentials();
        let config = Rtc::builder()
            .set_local_ice_credentials(IceCreds { ufrag: ufrag.clone(), pass })
            .set_dtls_cert(cert::libwebrtc_certificate()?);
        let mut rtc = Box::new(config.build(now));
        rtc.add_local_candidate(Candidate::host(local, "udp").map_err(|e| Error::Sdp(e.to_string()))?);
        let parsed = SdpOffer::from_sdp_string(&sdp::offer_for_str0m(offer)).map_err(|e| Error::Sdp(e.to_string()))?;
        let str0m_answer = rtc.sdp_api().accept_offer(parsed)?.to_sdp_string();
        let identity = server_identity(key, unix_now());
        let answer = sdp::vanilla_sdp(&str0m_answer, &identity_line(&str0m_answer, &identity), Some(local));
        let mut conn = Self::new(rtc, Some(local), ufrag, sdp::str0m_mid(&str0m_answer));
        if let Some(size) = sdp::max_message_size(offer)? {
            conn.max_message = size;
        }
        conn.drive(now);
        Ok((conn, answer))
    }
}
