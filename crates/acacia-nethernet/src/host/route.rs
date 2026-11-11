use std::net::SocketAddr;
use std::time::Instant;

use super::{Host, PeerId, MAX_ADDRS};
use crate::turn::stun::is_stun;

const HEADER: usize = 20;
const BINDING_REQUEST: [u8; 2] = [0x00, 0x01];
const USERNAME: u16 = 0x0006;

impl Host {
    /// The connection a datagram from `source` belongs to; `via` is the relay it came through.
    pub(super) fn route(&mut self, now: Instant, via: Option<SocketAddr>, source: SocketAddr, data: &[u8]) -> Option<PeerId> {
        let path = (via, source);
        let known = self.by_addr.get(&path).copied();
        // A binding request names its connection, which beats a stale mapping when an address is
        // reused; its credentials are checked first, so a spoofed one cannot take an address over.
        let named = request_ufrag(data).and_then(|ufrag| self.by_ufrag.get(ufrag).copied());
        let Some(id) = named.filter(|id| known != Some(*id)) else { return known };
        let peer = self.peers.get_mut(&id)?;
        let accepted = match via {
            None => peer.conn.accepts(now, source, data),
            Some(relay) => peer.conn.accepts_relayed(now, relay, source, data),
        };
        if peer.addrs.len() >= MAX_ADDRS || !accepted {
            return known;
        }
        peer.addrs.push(path);
        if let Some(old) = self.by_addr.insert(path, id).and_then(|old| self.peers.get_mut(&old)) {
            old.addrs.retain(|a| *a != path);
        }
        Some(id)
    }
}

/// The addresses of the `candidate:` lines in an SDP or a trickled line (`a=` optional):
/// `candidate:<foundation> <component> <proto> <priority> <ip> <port> typ …`. mDNS names are skipped.
pub(super) fn candidate_addrs(sdp: &str) -> impl Iterator<Item = SocketAddr> + '_ {
    sdp.lines().filter_map(|line| {
        let line = line.trim();
        let mut fields = line.strip_prefix("a=").unwrap_or(line).strip_prefix("candidate:")?.split_whitespace().skip(4);
        Some(SocketAddr::new(fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
    })
}

/// The local half of the USERNAME (`<ours>:<theirs>`) in an ICE binding request; `None` for any
/// other datagram.
pub(super) fn request_ufrag(data: &[u8]) -> Option<&str> {
    if !is_stun(data) || data[..2] != BINDING_REQUEST {
        return None;
    }
    let mut attrs = data.get(HEADER..)?;
    while let [t0, t1, l0, l1, rest @ ..] = attrs {
        let len = usize::from(u16::from_be_bytes([*l0, *l1]));
        let value = rest.get(..len)?;
        if u16::from_be_bytes([*t0, *t1]) == USERNAME {
            return std::str::from_utf8(value).ok()?.split(':').next();
        }
        // Values are padded to four bytes.
        attrs = rest.get(len.next_multiple_of(4)..)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn::stun::{Attr, Class, Message, Method};

    #[test]
    fn finds_the_local_ufrag_of_binding_requests_only() {
        let request = Message::new(Class::Request, Method::Binding, [7; 12])
            .with(Attr::Software("x".into()))
            .with(Attr::Username("Ab12:Zz99".into()))
            .encode(None, true);
        assert_eq!(request_ufrag(&request), Some("Ab12"));
        let response = Message::new(Class::Success, Method::Binding, [7; 12]).with(Attr::Username("Ab12:Zz99".into())).encode(None, false);
        assert_eq!(request_ufrag(&response), None);
        // Binding-request type bytes without the magic cookie: not STUN.
        assert_eq!(request_ufrag(&[0x00, 0x01, 0, 8, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 4]), None);
        assert_eq!(request_ufrag(&request[..request.len() - 30]), None);
    }

    #[test]
    fn reads_candidate_addresses_from_sdp_and_trickled_lines() {
        let sdp = "m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=candidate:1 1 udp 2113937151 192.168.1.2 5000 typ host generation 0\r\n\
                   a=candidate:2 1 udp 1 abc.local 5000 typ host\r\na=candidate:3 1 udp 1677729535 203.0.113.9 6000 typ srflx raddr 192.168.1.2 rport 5000\r\n";
        let addrs: Vec<_> = candidate_addrs(sdp).map(|a| a.to_string()).collect();
        assert_eq!(addrs, ["192.168.1.2:5000", "203.0.113.9:6000"]);
        assert_eq!(candidate_addrs("candidate:1 1 udp 1 [::1] 7 typ host").count(), 0);
        assert_eq!(candidate_addrs("candidate:1 1 udp 1 ::1 7 typ host").next(), Some("[::1]:7".parse().unwrap()));
    }
}
