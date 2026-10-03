//! STUN message codec (RFC 8489) with the TURN attributes and ChannelData framing (RFC 8656).

use std::net::SocketAddr;

use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use rand_core::{OsRng, RngCore};
use sha1::Sha1;

pub use super::attr::Attr;
use super::attr::{decode_attr, encode_attr, push_attr, FINGERPRINT, MESSAGE_INTEGRITY};
use super::TurnError;

pub const MAGIC_COOKIE: u32 = 0x2112_A442;
const HEADER: usize = 20;
const FINGERPRINT_XOR: u32 = 0x5354_554E;
const CRC32: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);

pub type TransactionId = [u8; 12];

pub fn transaction_id() -> TransactionId {
    let mut tid = [0; 12];
    OsRng.fill_bytes(&mut tid);
    tid
}

/// Long-term credential key: MD5(username:realm:password). Inputs are used as-is (no SASLprep).
pub fn long_term_key(username: &str, realm: &str, password: &str) -> [u8; 16] {
    Md5::digest(format!("{username}:{realm}:{password}")).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Request,
    Indication,
    Success,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Binding = 0x001,
    Allocate = 0x003,
    Refresh = 0x004,
    Send = 0x006,
    Data = 0x007,
    CreatePermission = 0x008,
    ChannelBind = 0x009,
}

impl Method {
    fn from_u16(m: u16) -> Option<Self> {
        use Method::*;
        [Binding, Allocate, Refresh, Send, Data, CreatePermission, ChannelBind].into_iter().find(|x| *x as u16 == m)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub class: Class,
    pub method: Method,
    pub transaction_id: TransactionId,
    /// Attributes before MESSAGE-INTEGRITY; later ones (except FINGERPRINT) are ignored on decode.
    pub attrs: Vec<Attr>,
}

macro_rules! getter {
    ($name:ident, $variant:ident, $ty:ty) => {
        pub fn $name(&self) -> Option<$ty> {
            self.attrs.iter().find_map(|a| match a {
                Attr::$variant(v) => Some(v.clone()),
                _ => None,
            })
        }
    };
}

impl Message {
    pub fn new(class: Class, method: Method, transaction_id: TransactionId) -> Self {
        Self { class, method, transaction_id, attrs: Vec::new() }
    }

    pub fn with(mut self, attr: Attr) -> Self {
        self.attrs.push(attr);
        self
    }

    getter!(xor_mapped, XorMappedAddress, SocketAddr);
    getter!(xor_peer, XorPeerAddress, SocketAddr);
    getter!(xor_relayed, XorRelayedAddress, SocketAddr);
    getter!(alternate_server, AlternateServer, SocketAddr);
    getter!(realm, Realm, String);
    getter!(nonce, Nonce, String);
    getter!(lifetime, Lifetime, u32);

    /// XOR-MAPPED-ADDRESS, else the legacy MAPPED-ADDRESS.
    pub fn mapped(&self) -> Option<SocketAddr> {
        self.xor_mapped().or_else(|| {
            self.attrs.iter().find_map(|a| match a {
                Attr::MappedAddress(v) => Some(*v),
                _ => None,
            })
        })
    }

    pub fn error_code(&self) -> Option<(u16, &str)> {
        self.attrs.iter().find_map(|a| match a {
            Attr::ErrorCode { code, reason } => Some((*code, reason.as_str())),
            _ => None,
        })
    }

    pub fn into_data(self) -> Option<Vec<u8>> {
        self.attrs.into_iter().find_map(|a| match a {
            Attr::Data(v) => Some(v),
            _ => None,
        })
    }

    /// Appends MESSAGE-INTEGRITY (HMAC-SHA1 with `integrity_key`) and FINGERPRINT when asked.
    pub fn encode(&self, integrity_key: Option<&[u8]>, fingerprint: bool) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(&message_type(self.method, self.class).to_be_bytes());
        buf.extend_from_slice(&[0, 0]);
        buf.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        buf.extend_from_slice(&self.transaction_id);
        for attr in &self.attrs {
            encode_attr(attr, &self.transaction_id, &mut buf);
        }
        if let Some(key) = integrity_key {
            set_length(&mut buf, 24);
            let mac = hmac_sha1(key, &buf);
            push_attr(&mut buf, MESSAGE_INTEGRITY, &mac);
        }
        if fingerprint {
            set_length(&mut buf, 8);
            let crc = CRC32.checksum(&buf) ^ FINGERPRINT_XOR;
            push_attr(&mut buf, FINGERPRINT, &crc.to_be_bytes());
        }
        set_length(&mut buf, 0);
        buf
    }

    /// Parses a message, rejecting a FINGERPRINT that doesn't match. Integrity is checked separately
    /// ([`verify_integrity`]) since the key depends on the transaction.
    pub fn decode(buf: &[u8]) -> Result<Self, TurnError> {
        let body = body(buf)?;
        let kind = u16::from_be_bytes([buf[0], buf[1]]);
        let class = match kind & 0x0110 {
            0x0000 => Class::Request,
            0x0010 => Class::Indication,
            0x0100 => Class::Success,
            _ => Class::Error,
        };
        let method = (kind & 0x000F) | ((kind >> 1) & 0x0070) | ((kind >> 2) & 0x0F80);
        let method = Method::from_u16(method).ok_or(TurnError::Stun("unknown method"))?;
        let transaction_id: TransactionId = buf[8..HEADER].try_into().unwrap();
        let mut msg = Self::new(class, method, transaction_id);
        let mut after_integrity = false;
        for item in attributes(body) {
            let (offset, kind, value) = item?;
            match kind {
                FINGERPRINT => {
                    let crc = covered_digest(buf, offset, 8, |parts| {
                        let mut d = CRC32.digest();
                        parts.iter().for_each(|p| d.update(p));
                        d.finalize()
                    });
                    if value != (crc ^ FINGERPRINT_XOR).to_be_bytes() {
                        return Err(TurnError::Stun("FINGERPRINT mismatch"));
                    }
                }
                MESSAGE_INTEGRITY => after_integrity = true,
                _ if after_integrity => {}
                _ => msg.attrs.extend(decode_attr(kind, value, &transaction_id)?),
            }
        }
        Ok(msg)
    }
}

/// Checks MESSAGE-INTEGRITY of an encoded message; false when it is missing or wrong.
pub fn verify_integrity(buf: &[u8], key: &[u8]) -> bool {
    let Ok(body) = body(buf) else { return false };
    attributes(body).map_while(Result::ok).find(|(_, kind, _)| *kind == MESSAGE_INTEGRITY).is_some_and(
        |(offset, _, value)| {
            let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC takes any key length");
            covered_digest(buf, offset, 24, |parts| parts.iter().for_each(|p| mac.update(p)));
            mac.verify_slice(value).is_ok()
        },
    )
}

/// True when `buf` looks like STUN (vs ChannelData or anything else on the socket).
pub fn is_stun(buf: &[u8]) -> bool {
    buf.len() >= HEADER && buf[0] & 0xC0 == 0 && buf[4..8] == MAGIC_COOKIE.to_be_bytes()
}

/// ChannelData framing; unpadded over UDP like libwebrtc (turn_port.cc `TurnEntry::Send`).
pub fn channel_data(channel: u16, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + data.len());
    buf.extend_from_slice(&channel.to_be_bytes());
    buf.extend_from_slice(&(data.len() as u16).to_be_bytes());
    buf.extend_from_slice(data);
    buf
}

pub fn parse_channel_data(buf: &[u8]) -> Option<(u16, &[u8])> {
    let [a, b, c, d, rest @ ..] = buf else { return None };
    let channel = u16::from_be_bytes([*a, *b]);
    let len = u16::from_be_bytes([*c, *d]) as usize;
    ((0x4000..=0x4FFF).contains(&channel) && len <= rest.len()).then(|| (channel, &rest[..len]))
}

fn message_type(method: Method, class: Class) -> u16 {
    let m = method as u16;
    let c = match class {
        Class::Request => 0x0000,
        Class::Indication => 0x0010,
        Class::Success => 0x0100,
        Class::Error => 0x0110,
    };
    (m & 0x000F) | ((m & 0x0070) << 1) | ((m & 0x0F80) << 2) | c
}

fn body(buf: &[u8]) -> Result<&[u8], TurnError> {
    if !is_stun(buf) {
        return Err(TurnError::Stun("not a STUN message"));
    }
    let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    if !len.is_multiple_of(4) || HEADER + len > buf.len() {
        return Err(TurnError::Stun("bad message length"));
    }
    Ok(&buf[HEADER..HEADER + len])
}

/// Yields `(offset in body, type, value)` for each attribute.
fn attributes(body: &[u8]) -> impl Iterator<Item = Result<(usize, u16, &[u8]), TurnError>> {
    let mut offset = 0;
    std::iter::from_fn(move || {
        let header = body.get(offset..offset + 4)?;
        let kind = u16::from_be_bytes([header[0], header[1]]);
        let len = u16::from_be_bytes([header[2], header[3]]) as usize;
        let Some(value) = body.get(offset + 4..offset + 4 + len) else {
            offset = body.len();
            return Some(Err(TurnError::Stun("attribute overruns message")));
        };
        let item = (offset, kind, value);
        offset += 4 + len.next_multiple_of(4);
        Some(Ok(item))
    })
}

/// Feeds the bytes a MESSAGE-INTEGRITY or FINGERPRINT at body `offset` covers, with the header length
/// rewritten to end just after that attribute (`attr_len` bytes including its header).
fn covered_digest<R>(buf: &[u8], offset: usize, attr_len: usize, digest: impl FnOnce(&[&[u8]]) -> R) -> R {
    let len = ((offset + attr_len) as u16).to_be_bytes();
    digest(&[&buf[..2], &len, &buf[4..HEADER + offset]])
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Sets the header length to the current body plus `extra` bytes about to be appended.
fn set_length(buf: &mut [u8], extra: usize) {
    let len = (buf.len() - HEADER + extra) as u16;
    buf[2..4].copy_from_slice(&len.to_be_bytes());
}
