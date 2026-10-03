//! Offers shaped like the vanilla client's (libwebrtc) and answers mapped back for str0m.
//! Layout reference: docs/research/vanilla-capture-2026-10-01.md.

use std::fmt::Write;
use std::net::SocketAddr;

use rand_core::{OsRng, RngCore};

/// libwebrtc priorities for Wi-Fi (local preference 0x1e; network-cost 10 = Wi-Fi). The relay value
/// carries libwebrtc's UDP-relay bits; it is from browser traces, not a vanilla capture.
const HOST_PRIORITY: u32 = 2113937151;
const SRFLX_PRIORITY: u32 = 1677729535;
const RELAY_PRIORITY: u32 = 41885439;
/// The vanilla client always bundles into mid `0`; str0m picks its own, so answers are mapped back.
const VANILLA_MID: &str = "0";

/// libwebrtc ICE credentials: 4-char ufrag and 24-char password from the base64 alphabet.
pub(crate) fn ice_credentials() -> (String, String) {
    (random_ice_chars(4), random_ice_chars(24))
}

fn random_ice_chars(n: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    (0..n).map(|_| ALPHABET[(OsRng.next_u32() % 64) as usize] as char).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateType {
    Host,
    /// Related address: the host base.
    ServerReflexive(SocketAddr),
    /// Related address: our address as the TURN server saw it.
    Relay(SocketAddr),
}

/// A libwebrtc-formatted `candidate:` line (no `a=`). Trickled candidates carry the ufrag; the one
/// embedded in a direct-connect offer does not.
pub(crate) fn candidate_line(addr: SocketAddr, typ: CandidateType, ufrag: Option<&str>) -> String {
    let (priority, name, related) = match typ {
        CandidateType::Host => (HOST_PRIORITY, "host", None),
        CandidateType::ServerReflexive(r) => (SRFLX_PRIORITY, "srflx", Some(r)),
        CandidateType::Relay(r) => (RELAY_PRIORITY, "relay", Some(r)),
    };
    let mut s = format!("candidate:{} 1 udp {priority} {} {} typ {name}", OsRng.next_u32(), addr.ip(), addr.port());
    if let Some(r) = related {
        let _ = write!(s, " raddr {} rport {}", r.ip(), r.port());
    }
    s.push_str(" generation 0");
    if let Some(ufrag) = ufrag {
        let _ = write!(s, " ufrag {ufrag}");
    }
    s.push_str(" network-id 1 network-cost 10");
    s
}

/// Rebuilds str0m's offer or answer in the vanilla line order (BDS answers use the same one) with
/// `identity_line` at session level. Takes the fingerprint, ICE credentials and DTLS setup from
/// str0m's SDP. With `embedded` the SDP carries that host candidate (direct connect); without,
/// candidates are trickled and the media line holds libwebrtc's pre-gathering placeholders.
pub(crate) fn vanilla_sdp(str0m_offer: &str, identity_line: &str, embedded: Option<SocketAddr>) -> String {
    let attr = |name: &str| str0m_offer.lines().find_map(|l| l.strip_prefix(name)).unwrap_or_default();
    let placeholder = SocketAddr::from(([0, 0, 0, 0], 9));
    let media = embedded.unwrap_or(placeholder);
    let family = if media.is_ipv4() { "IP4" } else { "IP6" };
    let mut s = String::with_capacity(str0m_offer.len() + identity_line.len() + 256);
    let mut line = |l: &str| {
        s.push_str(l);
        s.push_str("\r\n");
    };
    line("v=0");
    line(&format!("o=- {} 2 IN IP4 127.0.0.1", OsRng.next_u64() >> 1));
    line("s=-");
    line("t=0 0");
    line(&format!("a=group:BUNDLE {VANILLA_MID}"));
    line("a=extmap-allow-mixed");
    line("a=msid-semantic: WMS");
    line(identity_line);
    line(&format!("m=application {} UDP/DTLS/SCTP webrtc-datachannel", media.port()));
    line(&format!("c=IN {family} {}", media.ip()));
    if let Some(host) = embedded {
        line(&format!("a={}", candidate_line(host, CandidateType::Host, None)));
    }
    line(&format!("a=ice-ufrag:{}", attr("a=ice-ufrag:")));
    line(&format!("a=ice-pwd:{}", attr("a=ice-pwd:")));
    line("a=ice-options:trickle");
    for fingerprint in str0m_offer.lines().filter(|l| l.starts_with("a=fingerprint:")) {
        line(fingerprint);
    }
    line(&format!("a=setup:{}", attr("a=setup:")));
    line(&format!("a=mid:{VANILLA_MID}"));
    line("a=sctp-port:5000");
    line("a=max-message-size:262144");
    s
}

/// The mid str0m gave its data section, needed to map the answer back.
pub(crate) fn str0m_mid(str0m_offer: &str) -> String {
    str0m_offer.lines().find_map(|l| l.strip_prefix("a=mid:")).unwrap_or_default().to_owned()
}

/// The peer's `a=max-message-size`, if it sent one.
pub(crate) fn max_message_size(sdp: &str) -> Result<Option<usize>, crate::Error> {
    sdp.lines()
        .find_map(|l| l.strip_prefix("a=max-message-size:"))
        .map(|v| v.trim().parse().map_err(|_| crate::Error::Sdp("bad max-message-size".into())))
        .transpose()
}

/// An offer as str0m should answer it: no `a=identity`, and `actpass` narrowed to `passive` so str0m
/// takes the DTLS client role (`a=setup:active`) as BDS does; given `actpass` it picks `passive`.
pub(crate) fn offer_for_str0m(offer: &str) -> String {
    let mut out = String::with_capacity(offer.len());
    for l in offer.lines().filter(|l| !l.starts_with(crate::identity::IDENTITY)) {
        let _ = write!(out, "{}\r\n", if l == "a=setup:actpass" { "a=setup:passive" } else { l });
    }
    out
}

/// The answer as str0m expects it: no `a=identity`, and our mid instead of the vanilla one.
pub(crate) fn answer_for_str0m(answer: &str, mid: &str) -> String {
    let mut out = String::with_capacity(answer.len());
    for l in answer.lines().filter(|l| !l.starts_with(crate::identity::IDENTITY)) {
        let mapped = match l {
            l if l == format!("a=mid:{VANILLA_MID}") => format!("a=mid:{mid}"),
            l if l == format!("a=group:BUNDLE {VANILLA_MID}") => format!("a=group:BUNDLE {mid}"),
            l => l.to_owned(),
        };
        let _ = write!(out, "{mapped}\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const STR0M: &str = "v=0\r\no=str0m-0.24.0 1 2 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=group:BUNDLE xYz\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=ice-ufrag:Ab12\r\na=ice-pwd:abcdefghijklmnopqrstuvwx\r\na=fingerprint:sha-256 AA:BB\r\na=setup:actpass\r\na=mid:xYz\r\n";

    #[test]
    fn offer_follows_vanilla_layout() {
        let offer = vanilla_sdp(STR0M, "a=identity:e30=", Some("192.168.1.20:50123".parse().unwrap()));
        let key = |l: &str| match l.strip_prefix("a=") {
            Some(attr) => format!("a={}", attr.split(':').next().unwrap_or_default()),
            None if l.starts_with("m=") || l.starts_with("c=") => l.to_owned(),
            None => l[..2].to_owned(),
        };
        let keys: Vec<String> = offer.lines().map(key).collect();
        assert_eq!(
            keys,
            ["v=", "o=", "s=", "t=", "a=group", "a=extmap-allow-mixed", "a=msid-semantic", "a=identity", "m=application 50123 UDP/DTLS/SCTP webrtc-datachannel", "c=IN IP4 192.168.1.20", "a=candidate", "a=ice-ufrag", "a=ice-pwd", "a=ice-options", "a=fingerprint", "a=setup", "a=mid", "a=sctp-port", "a=max-message-size"]
        );
        let origin = offer.lines().nth(1).unwrap();
        assert!(origin.starts_with("o=- ") && origin.ends_with(" 2 IN IP4 127.0.0.1"));
        assert!(!offer.contains("str0m"));
        assert!(offer.contains("a=mid:0\r\n") && offer.contains("a=fingerprint:sha-256 AA:BB\r\n"));
        assert!(offer.contains(" 192.168.1.20 50123 typ host generation 0 network-id 1 network-cost 10\r\n"));
    }

    #[test]
    fn trickle_offer_has_no_candidates() {
        let offer = vanilla_sdp(STR0M, "a=identity:e30=", None);
        assert!(!offer.contains("a=candidate"));
        assert!(offer.contains("m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\nc=IN IP4 0.0.0.0\r\na=ice-ufrag:Ab12\r\n"));
    }

    #[test]
    fn candidate_lines_match_libwebrtc() {
        let host: SocketAddr = "192.168.1.2:5000".parse().unwrap();
        let tail = |l: String| l.split_once(' ').unwrap().1.to_owned();
        assert_eq!(
            tail(candidate_line("1.2.3.4:6000".parse().unwrap(), CandidateType::ServerReflexive(host), Some("Ab12"))),
            "1 udp 1677729535 1.2.3.4 6000 typ srflx raddr 192.168.1.2 rport 5000 generation 0 ufrag Ab12 network-id 1 network-cost 10"
        );
        assert_eq!(
            tail(candidate_line(host, CandidateType::Host, None)),
            "1 udp 2113937151 192.168.1.2 5000 typ host generation 0 network-id 1 network-cost 10"
        );
        assert!(candidate_line(host, CandidateType::Host, None).starts_with("candidate:"));
    }

    #[test]
    fn answer_maps_mid_back_and_drops_identity() {
        let answer = "a=group:BUNDLE 0\r\na=identity:xyz\r\na=mid:0\r\n";
        assert_eq!(answer_for_str0m(answer, "xYz"), "a=group:BUNDLE xYz\r\na=mid:xYz\r\n");
        let (ufrag, pwd) = ice_credentials();
        assert_eq!((ufrag.len(), pwd.len()), (4, 24));
    }
}
