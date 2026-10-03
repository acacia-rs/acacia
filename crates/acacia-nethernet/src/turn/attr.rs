//! STUN attribute values (RFC 8489 §14, RFC 8656 §18).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::stun::{TransactionId, MAGIC_COOKIE};
use super::TurnError;

const MAPPED_ADDRESS: u16 = 0x0001;
const USERNAME: u16 = 0x0006;
pub(super) const MESSAGE_INTEGRITY: u16 = 0x0008;
const ERROR_CODE: u16 = 0x0009;
const CHANNEL_NUMBER: u16 = 0x000C;
const LIFETIME: u16 = 0x000D;
const XOR_PEER_ADDRESS: u16 = 0x0012;
const DATA: u16 = 0x0013;
const REALM: u16 = 0x0014;
const NONCE: u16 = 0x0015;
const XOR_RELAYED_ADDRESS: u16 = 0x0016;
const REQUESTED_TRANSPORT: u16 = 0x0019;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
const SOFTWARE: u16 = 0x8022;
const ALTERNATE_SERVER: u16 = 0x8023;
pub(super) const FINGERPRINT: u16 = 0x8028;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attr {
    MappedAddress(SocketAddr),
    XorMappedAddress(SocketAddr),
    XorPeerAddress(SocketAddr),
    XorRelayedAddress(SocketAddr),
    /// Where a `300 Try Alternate` redirects (plain, not XOR'd).
    AlternateServer(SocketAddr),
    Username(String),
    Realm(String),
    Nonce(String),
    ErrorCode { code: u16, reason: String },
    Lifetime(u32),
    /// IANA protocol number; 17 is UDP.
    RequestedTransport(u8),
    ChannelNumber(u16),
    Data(Vec<u8>),
    Software(String),
    /// Encoded verbatim; the decoder skips unknown types instead of producing this.
    Other { kind: u16, value: Vec<u8> },
}

pub(super) fn push_attr(buf: &mut Vec<u8>, kind: u16, value: &[u8]) {
    buf.extend_from_slice(&kind.to_be_bytes());
    buf.extend_from_slice(&(value.len() as u16).to_be_bytes());
    buf.extend_from_slice(value);
    buf.resize(buf.len().next_multiple_of(4), 0);
}

pub(super) fn encode_attr(attr: &Attr, tid: &TransactionId, buf: &mut Vec<u8>) {
    match attr {
        Attr::MappedAddress(a) => push_attr(buf, MAPPED_ADDRESS, &address(*a, None)),
        Attr::XorMappedAddress(a) => push_attr(buf, XOR_MAPPED_ADDRESS, &address(*a, Some(tid))),
        Attr::XorPeerAddress(a) => push_attr(buf, XOR_PEER_ADDRESS, &address(*a, Some(tid))),
        Attr::XorRelayedAddress(a) => push_attr(buf, XOR_RELAYED_ADDRESS, &address(*a, Some(tid))),
        Attr::AlternateServer(a) => push_attr(buf, ALTERNATE_SERVER, &address(*a, None)),
        Attr::Username(s) => push_attr(buf, USERNAME, s.as_bytes()),
        Attr::Realm(s) => push_attr(buf, REALM, s.as_bytes()),
        Attr::Nonce(s) => push_attr(buf, NONCE, s.as_bytes()),
        Attr::Software(s) => push_attr(buf, SOFTWARE, s.as_bytes()),
        Attr::ErrorCode { code, reason } => {
            let mut v = vec![0, 0, (code / 100) as u8, (code % 100) as u8];
            v.extend_from_slice(reason.as_bytes());
            push_attr(buf, ERROR_CODE, &v);
        }
        Attr::Lifetime(s) => push_attr(buf, LIFETIME, &s.to_be_bytes()),
        Attr::RequestedTransport(p) => push_attr(buf, REQUESTED_TRANSPORT, &[*p, 0, 0, 0]),
        Attr::ChannelNumber(n) => push_attr(buf, CHANNEL_NUMBER, &(u32::from(*n) << 16).to_be_bytes()),
        Attr::Data(d) => push_attr(buf, DATA, d),
        Attr::Other { kind, value } => push_attr(buf, *kind, value),
    }
}

/// `Ok(None)` for attribute types this codec doesn't know.
pub(super) fn decode_attr(kind: u16, v: &[u8], tid: &TransactionId) -> Result<Option<Attr>, TurnError> {
    let text = |v: &[u8]| String::from_utf8(v.to_vec()).map_err(|_| TurnError::Stun("attribute is not UTF-8"));
    let word = |v: &[u8]| v.try_into().map(u32::from_be_bytes).map_err(|_| TurnError::Stun("bad 32-bit attribute"));
    Ok(Some(match kind {
        MAPPED_ADDRESS => Attr::MappedAddress(parse_address(v, None)?),
        XOR_MAPPED_ADDRESS => Attr::XorMappedAddress(parse_address(v, Some(tid))?),
        XOR_PEER_ADDRESS => Attr::XorPeerAddress(parse_address(v, Some(tid))?),
        XOR_RELAYED_ADDRESS => Attr::XorRelayedAddress(parse_address(v, Some(tid))?),
        ALTERNATE_SERVER => Attr::AlternateServer(parse_address(v, None)?),
        USERNAME => Attr::Username(text(v)?),
        REALM => Attr::Realm(text(v)?),
        NONCE => Attr::Nonce(text(v)?),
        SOFTWARE => Attr::Software(text(v)?),
        ERROR_CODE => {
            let [_, _, class, number, reason @ ..] = v else { return Err(TurnError::Stun("short ERROR-CODE")) };
            let code = u16::from(class & 0x7) * 100 + u16::from(*number);
            Attr::ErrorCode { code, reason: String::from_utf8_lossy(reason).into_owned() }
        }
        LIFETIME => Attr::Lifetime(word(v)?),
        REQUESTED_TRANSPORT => Attr::RequestedTransport((word(v)? >> 24) as u8),
        CHANNEL_NUMBER => Attr::ChannelNumber((word(v)? >> 16) as u16),
        DATA => Attr::Data(v.to_vec()),
        _ => return Ok(None),
    }))
}

/// XOR mask for an address: the cookie, then the transaction id for the IPv6 tail.
fn mask(tid: &TransactionId) -> [u8; 16] {
    let mut m = [0; 16];
    m[..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    m[4..].copy_from_slice(tid);
    m
}

fn address(addr: SocketAddr, xor: Option<&TransactionId>) -> Vec<u8> {
    let m = xor.map_or([0; 16], mask);
    let (family, ip) = match addr.ip() {
        IpAddr::V4(ip) => (1, ip.octets().to_vec()),
        IpAddr::V6(ip) => (2, ip.octets().to_vec()),
    };
    let port = addr.port() ^ u16::from_be_bytes([m[0], m[1]]);
    let mut v = vec![0, family];
    v.extend_from_slice(&port.to_be_bytes());
    v.extend(ip.iter().zip(m).map(|(b, k)| b ^ k));
    v
}

fn parse_address(v: &[u8], xor: Option<&TransactionId>) -> Result<SocketAddr, TurnError> {
    let m = xor.map_or([0; 16], mask);
    let [_, family, p0, p1, ip @ ..] = v else { return Err(TurnError::Stun("short address")) };
    let port = u16::from_be_bytes([*p0, *p1]) ^ u16::from_be_bytes([m[0], m[1]]);
    let mut octets = [0u8; 16];
    for (o, (b, k)) in octets.iter_mut().zip(ip.iter().zip(m)) {
        *o = b ^ k;
    }
    let ip = match (family, ip.len()) {
        (1, 4) => IpAddr::V4(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3])),
        (2, 16) => IpAddr::V6(Ipv6Addr::from(octets)),
        _ => return Err(TurnError::Stun("bad address family")),
    };
    Ok(SocketAddr::new(ip, port))
}
