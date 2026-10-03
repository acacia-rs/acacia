//! Hand-built values survive encode → decode, and the header/`RawPacket` API behaves.

use acacia_proto::packets::*;
use acacia_proto::types::{LoginTokens, Vec3f};
use acacia_proto::{DecodeError, Packet, RawPacket, encode_packet, nbt};
use bytes::{Bytes, BytesMut};

fn roundtrip<T: Packet + std::fmt::Debug + PartialEq>(p: &T) -> Bytes {
    let mut out = BytesMut::new();
    encode_packet(p, &mut out);
    let bytes = out.freeze();
    let raw = RawPacket::parse(bytes.clone()).unwrap();
    assert!(raw.is::<T>());
    assert_eq!(&raw.decode::<T>().unwrap(), p);
    bytes
}

#[test]
fn simple_packets() {
    let bytes = roundtrip(&SetTime { time: -1234 });
    assert_eq!(&bytes[..], &[10, 0xa3, 0x13]);
    roundtrip(&PlayStatus {
        status: PlayStatusStatus::PlayerSpawn,
    });
    roundtrip(&RequestNetworkSettings {
        client_protocol: acacia_proto::PROTOCOL_VERSION,
    });
    roundtrip(&ServerToClientHandshake {
        token: "jwt.header.sig".into(),
    });
    roundtrip(&ClientToServerHandshake {});
    roundtrip(&RequestChunkRadius {
        chunk_radius: 12,
        max_radius: 32,
    });
    roundtrip(&SetLocalPlayerAsInitialized {
        runtime_entity_id: u64::MAX,
    });
}

#[test]
fn login_with_encapsulated_tokens() {
    let tokens = LoginTokens {
        identity: "{\"chain\":[]}".into(),
        client: "a.b.c".into(),
    };
    roundtrip(&Login {
        protocol_version: 2193,
        tokens: Some(tokens),
    });
    roundtrip(&Login {
        protocol_version: 2193,
        tokens: None,
    });
}

#[test]
fn switches_and_options() {
    let chat = Text {
        needs_translation: false,
        category: TextCategory::Authored,
        r#type: TextType::Chat,
        content: TextContent::Chat(TextContentChat {
            source_name: "Steve".into(),
            message: "hi §a".into(),
        }),
        xuid: "123".into(),
        platform_chat_id: String::new(),
        has_filtered_message: true,
        filtered_message: Some("hi".into()),
    };
    roundtrip(&chat);
    roundtrip(&Disconnect {
        reason: acacia_proto::types::DisconnectFailReason::Kicked,
        hide_disconnect_reason: false,
        content: Some(DisconnectContent {
            message: "bye".into(),
            filtered_message: String::new(),
        }),
    });
    roundtrip(&ResourcePackClientResponse {
        response_status: ResourcePackClientResponseResponseStatus::Completed,
        response_status_name: String::new(),
        resourcepackids: None,
    });
    roundtrip(&Interact {
        action_id: InteractActionId::OpenInventory,
        target_entity_id: 1,
        has_position: true,
        position: Some(Vec3f {
            x: 1.5,
            y: -2.0,
            z: 0.25,
        }),
    });
}

#[test]
fn unknown_mapper_values_survive() {
    roundtrip(&PlayStatus {
        status: PlayStatusStatus::Unknown(99),
    });
}

#[test]
fn raw_packet_header_and_errors() {
    // id 10 with sender subclient 1 and target subclient 2: 10 | 1 << 10 | 2 << 12 = 9226.
    let mut buf = BytesMut::new();
    acacia_proto::codec::write_varint(&mut buf, 9226);
    acacia_proto::codec::write_zigzag32(&mut buf, 7);
    let raw = RawPacket::parse(buf.freeze()).unwrap();
    assert_eq!(
        (raw.id, raw.sender_subclient, raw.target_subclient),
        (10, 1, 2)
    );
    assert_eq!(raw.decode::<SetTime>().unwrap().time, 7);
    assert!(matches!(
        raw.decode::<PlayStatus>(),
        Err(DecodeError::IdMismatch {
            expected: 2,
            actual: 10
        })
    ));

    let truncated = RawPacket {
        id: 2,
        sender_subclient: 0,
        target_subclient: 0,
        body: Bytes::from_static(&[0, 0]),
    };
    let err = truncated.decode::<PlayStatus>().unwrap_err();
    assert!(matches!(err.root(), DecodeError::Eof { .. }));
    assert_eq!(
        err.to_string(),
        "play_status: PlayStatus.status: unexpected end of input (needed 4 bytes, 2 remaining)"
    );
}

#[test]
fn nbt_both_flavours() {
    let value = nbt::Value::Compound(vec![
        ("name".into(), nbt::Value::String("minecraft:stone".into())),
        ("n".into(), nbt::Value::Int(-5)),
        (
            "l".into(),
            nbt::Value::List(nbt::List {
                tag: 10,
                items: vec![],
            }),
        ),
        ("a".into(), nbt::Value::LongArray(vec![1, -1])),
    ]);
    let root = nbt::Nbt {
        name: String::new(),
        value,
    };
    for le in [false, true] {
        let mut w = BytesMut::new();
        if le {
            nbt::write::<nbt::LittleEndian>(&mut w, &root)
        } else {
            nbt::write::<nbt::Network>(&mut w, &root)
        }
        let (mut a, mut b) = (&w[..], &w[..]);
        let back = if le {
            nbt::read::<nbt::LittleEndian>(&mut a)
        } else {
            nbt::read::<nbt::Network>(&mut a)
        }
        .unwrap();
        assert_eq!(back, root);
        if le {
            nbt::skip::<nbt::LittleEndian>(&mut b)
        } else {
            nbt::skip::<nbt::Network>(&mut b)
        }
        .unwrap();
        assert!(a.is_empty() && b.is_empty());
    }
}
