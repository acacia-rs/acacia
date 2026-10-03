use std::net::SocketAddr;

use super::stun::*;

fn hex(s: &str) -> Vec<u8> {
    let digits: Vec<u8> = s.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits.chunks(2).map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap()).collect()
}

const SHORT_TERM: &[u8] = b"VOkJxbRl1RmTxUk/WvJxBt";
const TID: TransactionId = [0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae];

/// RFC 5769 §2.1.
const REQUEST: &str = "00010058 2112a442 b7e7a701 bc34d686 fa87dfae
    80220010 5354554e 20746573 7420636c 69656e74 00240004 6e0001ff 80290008 932ff9b1 51263b36
    00060009 6576746a 3a683676 59202020 00080014 9aeaa70c bfd8cb56 781ef2b5 b2d3f249 c1b571a2
    80280004 e57a3bcf";
/// §2.2.
const RESPONSE_V4: &str = "0101003c 2112a442 b7e7a701 bc34d686 fa87dfae
    8022000b 74657374 20766563 746f7220 00200008 0001a147 e112a643
    00080014 2b91f599 fd9e90c3 8c7489f9 2af9ba53 f06be7d7 80280004 c07d4c96";
/// §2.3.
const RESPONSE_V6: &str = "01010048 2112a442 b7e7a701 bc34d686 fa87dfae
    8022000b 74657374 20766563 746f7220 00200014 0002a147 0113a9fa a5d3f179 bc25f4b5 bed2b9d9
    00080014 a382954e 4be67bf1 1784c97c 8292c275 bfe3ed41 80280004 c8fb0b4c";
/// §2.4.
const LONG_TERM_REQUEST: &str = "00010060 2112a442 78ad3433 c6ad72c0 29da412e
    00060012 e3839ee3 8388e383 aae38383 e382afe3 82b90000
    0015001c 662f2f34 39396b39 35346436 4f4c3334 6f4c3946 53547679 36347341
    0014000b 6578616d 706c652e 6f726700
    00080014 f6702465 6dd64a3e 02b8e071 2e85c9a2 8ca89666";

#[test]
fn rfc5769_short_term_request() {
    let raw = hex(REQUEST);
    let msg = Message::decode(&raw).unwrap();
    assert_eq!((msg.class, msg.method, msg.transaction_id), (Class::Request, Method::Binding, TID));
    assert_eq!(msg.attrs, [Attr::Software("STUN test client".into()), Attr::Username("evtj:h6vY".into())]);
    assert!(verify_integrity(&raw, SHORT_TERM));
    assert!(!verify_integrity(&raw, b"wrong"));
}

#[test]
fn rfc5769_xor_mapped_responses() {
    for (vector, addr) in [
        (RESPONSE_V4, "192.0.2.1:32853"),
        (RESPONSE_V6, "[2001:db8:1234:5678:11:2233:4455:6677]:32853"),
    ] {
        let raw = hex(vector);
        let msg = Message::decode(&raw).unwrap();
        let addr: SocketAddr = addr.parse().unwrap();
        assert_eq!((msg.class, msg.mapped()), (Class::Success, Some(addr)));
        assert!(verify_integrity(&raw, SHORT_TERM));
        // The vectors pad SOFTWARE with a space (byte 35), which the MAC and CRC then cover.
        let ours = msg.encode(Some(SHORT_TERM), true);
        assert_eq!(ours.len(), raw.len());
        assert_eq!(ours[..35], raw[..35]);
        assert_eq!(ours[36..raw.len() - 32], raw[36..raw.len() - 32]);
        assert!(verify_integrity(&ours, SHORT_TERM));
    }
}

#[test]
fn rfc5769_long_term_request_encodes_byte_exact() {
    let raw = hex(LONG_TERM_REQUEST);
    let key = long_term_key("\u{30DE}\u{30C8}\u{30EA}\u{30C3}\u{30AF}\u{30B9}", "example.org", "TheMatrIX");
    assert!(verify_integrity(&raw, &key));
    let msg = Message::decode(&raw).unwrap();
    assert_eq!(msg.nonce().as_deref(), Some("f//499k954d6OL34oL9FSTvy64sA"));
    assert_eq!(msg.realm().as_deref(), Some("example.org"));
    assert_eq!(msg.encode(Some(&key), false), raw);
}

#[test]
fn turn_attributes_roundtrip() {
    let tid = transaction_id();
    let msg = Message::new(Class::Error, Method::ChannelBind, tid)
        .with(Attr::MappedAddress("10.0.0.1:1".parse().unwrap()))
        .with(Attr::XorPeerAddress("[::1]:9".parse().unwrap()))
        .with(Attr::XorRelayedAddress("198.51.100.2:49152".parse().unwrap()))
        .with(Attr::ErrorCode { code: 438, reason: "Stale Nonce".into() })
        .with(Attr::Lifetime(600))
        .with(Attr::RequestedTransport(17))
        .with(Attr::ChannelNumber(0x4001))
        .with(Attr::Data(vec![1, 2, 3, 4, 5]))
        .with(Attr::Realm("r".into()))
        .with(Attr::Nonce("n".into()));
    for (key, fingerprint) in [(None, false), (Some(&b"k"[..]), true)] {
        let raw = msg.encode(key, fingerprint);
        assert_eq!(Message::decode(&raw).unwrap(), msg);
        assert_eq!(key.is_some_and(|k| verify_integrity(&raw, k)), key.is_some());
    }
    assert_eq!(msg.error_code(), Some((438, "Stale Nonce")));
}

#[test]
fn skips_unknown_and_post_integrity_attributes() {
    let msg = Message::new(Class::Indication, Method::Data, transaction_id())
        .with(Attr::Other { kind: 0x7FFF, value: vec![9; 5] })
        .with(Attr::Lifetime(1));
    let mut raw = msg.encode(Some(b"k"), false);
    assert_eq!(Message::decode(&raw).unwrap().attrs, [Attr::Lifetime(1)]);
    raw.extend_from_slice(&hex("000d0004 00000002"));
    let len = (raw.len() - 20) as u16;
    raw[2..4].copy_from_slice(&len.to_be_bytes());
    assert_eq!(Message::decode(&raw).unwrap().lifetime(), Some(1));
}

#[test]
fn rejects_corrupt_messages() {
    let mut raw = hex(RESPONSE_V4);
    assert!(Message::decode(&raw[..raw.len() - 4]).is_err());
    let last = raw.len() - 1;
    raw[last] ^= 1;
    assert!(Message::decode(&raw).is_err());
    raw[last] ^= 1;
    raw[30] ^= 1;
    assert!(Message::decode(&raw).is_err());
    assert!(!verify_integrity(&raw, SHORT_TERM));
    assert!(Message::decode(&[0; 20]).is_err());
}

#[test]
fn channel_data_framing() {
    let framed = channel_data(0x4000, b"abcde");
    assert_eq!(framed, hex("4000 0005 6162636465"));
    assert!(!is_stun(&framed));
    assert_eq!(parse_channel_data(&framed), Some((0x4000, &b"abcde"[..])));
    let mut padded = framed.clone();
    padded.extend([0, 0, 0]);
    assert_eq!(parse_channel_data(&padded), Some((0x4000, &b"abcde"[..])));
    assert_eq!(parse_channel_data(&framed[..8]), None);
    assert_eq!(parse_channel_data(&channel_data(0x3FFF, b"x")), None);
}
