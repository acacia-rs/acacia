//! `decode_strict` rejects what `decode` lets through (docs/proto.md, "Strict decoding").

use acacia_proto::packets::{ClientCacheStatus, PlayStatus, PlayStatusStatus, PlayerList, SetTime};
use acacia_proto::strict::{Leniency, check};
use acacia_proto::{DecodeError, Packet, RawPacket, encode_packet};
use bytes::{BufMut, BytesMut};

fn encoded<T: Packet>(p: &T) -> BytesMut {
    let mut out = BytesMut::new();
    encode_packet(p, &mut out);
    out
}

fn raw(bytes: BytesMut) -> RawPacket {
    RawPacket::parse(bytes.freeze()).unwrap()
}

fn body(id: u32, body: &[u8]) -> RawPacket {
    RawPacket { id, sender_subclient: 0, target_subclient: 0, body: body.to_vec().into() }
}

#[test]
fn exact_packets_pass() {
    let packet = raw(encoded(&SetTime { time: 6000 }));
    assert_eq!(packet.decode_strict::<SetTime>().unwrap(), SetTime { time: 6000 });
    assert_eq!(check(&packet), Ok(()));
}

#[test]
fn trailing_bytes_are_an_error() {
    let mut bytes = encoded(&SetTime { time: 6000 });
    bytes.put_slice(&[1, 2]);
    let packet = raw(bytes);
    assert!(packet.decode::<SetTime>().is_ok());
    let err = check(&packet).unwrap_err();
    assert_eq!(err.root(), &DecodeError::TrailingBytes(2));
    assert_eq!(err.to_string(), "set_time: 2 bytes after the packet body");
}

#[test]
fn unknown_enum_values_are_an_error() {
    let packet = raw(encoded(&PlayStatus { status: PlayStatusStatus::Unknown(99) }));
    assert!(packet.decode::<PlayStatus>().is_ok());
    let leniency = Leniency::UnknownEnum { ty: "PlayStatusStatus", value: 99 };
    assert_eq!(check(&packet).unwrap_err().root(), &DecodeError::Lenient(leniency));
}

#[test]
fn a_leniency_does_not_leak_into_the_next_decode() {
    let lenient = raw(encoded(&PlayStatus { status: PlayStatusStatus::Unknown(99) }));
    lenient.decode::<PlayStatus>().unwrap();
    assert_eq!(check(&raw(encoded(&SetTime { time: 1 }))), Ok(()));
}

#[test]
fn bool_bytes_other_than_0_and_1_are_an_error() {
    let packet = body(ClientCacheStatus::ID, &[2]);
    assert_eq!(packet.decode::<ClientCacheStatus>(), Ok(ClientCacheStatus { enabled: true }));
    assert_eq!(check(&packet).unwrap_err().root(), &DecodeError::Lenient(Leniency::Bool(2)));
}

#[test]
fn a_list_shorter_than_its_count_is_an_error() {
    // A record count of 2 with no records after it.
    let packet = body(PlayerList::ID, &[2]);
    assert!(packet.decode::<PlayerList>().is_ok());
    assert_eq!(check(&packet).unwrap_err().root(), &DecodeError::Lenient(Leniency::TruncatedList));
}

#[test]
fn unknown_packet_ids_are_an_error() {
    assert_eq!(check(&body(999, &[])), Err(DecodeError::UnknownPacket(999)));
}
