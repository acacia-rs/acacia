use acacia_proto::packets::{NetworkSettingsCompressionAlgorithm, SetTime, Text, TextCategory, TextContent, TextContentRaw, TextType};

use super::*;
use crate::intercept::{encode, Interceptor, Verdict};

fn text(message: &str) -> Text {
    Text {
        needs_translation: false,
        category: TextCategory::MessageOnly,
        r#type: TextType::Raw,
        content: TextContent::Raw(TextContentRaw { message: message.into() }),
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    }
}

fn message(packet: &RawPacket) -> String {
    match packet.decode::<Text>().expect("a Text").content {
        TextContent::Raw(c) => c.message,
        other => panic!("{other:?}"),
    }
}

/// Upper-cases Text from the server, drops SetTime, and tags the game's Text with a second one.
struct Shout;

impl Interceptor for Shout {
    fn on_server_packet(&mut self, packet: &RawPacket) -> Verdict {
        if packet.is::<SetTime>() {
            return Verdict::Drop;
        }
        if !packet.is::<Text>() {
            return Verdict::Forward;
        }
        Verdict::replace(&text(&message(packet).to_uppercase()))
    }

    fn on_game_packet(&mut self, packet: &RawPacket) -> Verdict {
        if !packet.is::<Text>() {
            return Verdict::Forward;
        }
        Verdict::Replace(vec![encode(&text(&message(packet))), encode(&text("[via proxy]"))])
    }
}

/// Appends `!` to Text, to show it sees the previous interceptor's output.
struct Exclaim;

impl Interceptor for Exclaim {
    fn on_server_packet(&mut self, packet: &RawPacket) -> Verdict {
        if !packet.is::<Text>() {
            return Verdict::Forward;
        }
        Verdict::replace(&text(&format!("{}!", message(packet))))
    }
}

fn relay(interceptors: Vec<Box<dyn Interceptor>>) -> Relay {
    Relay::new(Wire::RakNet, SigningKey::random(&mut rand_core::OsRng), None, None, Chain::new(interceptors))
}

fn decode(codec: &mut BatchCodec, batch: &[u8]) -> Vec<RawPacket> {
    let mut packets = Vec::new();
    codec.decode(batch, &mut packets).expect("a valid batch");
    packets.into_iter().map(|p| RawPacket::parse(p).expect("a header")).collect()
}

fn batch(codec: &mut BatchCodec, packets: &[Bytes]) -> Bytes {
    codec.encode(packets.iter().map(|p| &p[..]))
}

#[test]
fn no_interceptors_pass_packets_through_unchanged() {
    let mut relay = relay(Vec::new());
    let packets = [encode(&text("hi")), encode(&SetTime { time: 5 })];
    let out = relay.on_server_message(&batch(&mut BatchCodec::default(), &packets)).unwrap();
    assert_eq!(out.to_game, [batch(&mut BatchCodec::default(), &packets)]);
}

#[test]
fn chain_rewrites_drops_and_feeds_the_next_interceptor() {
    let mut relay = relay(vec![Box::new(Shout), Box::new(Exclaim)]);
    let msg = batch(&mut BatchCodec::default(), &[encode(&SetTime { time: 5 }), encode(&text("hi"))]);
    let out = relay.on_server_message(&msg).unwrap();
    let got = decode(&mut BatchCodec::default(), &out.to_game[0]);
    assert_eq!(got.iter().map(message).collect::<Vec<_>>(), ["HI!"]);
}

#[test]
fn a_fully_dropped_batch_sends_nothing() {
    let mut relay = relay(vec![Box::new(Shout)]);
    let out = relay.on_server_message(&batch(&mut BatchCodec::default(), &[encode(&SetTime { time: 5 })])).unwrap();
    assert!(out.to_game.is_empty() && out.to_server.is_empty());
}

#[test]
fn rewritten_packets_are_recompressed_per_side_after_network_settings() {
    let mut relay = relay(vec![Box::new(Shout)]);
    let settings = NetworkSettings {
        compression_threshold: 1,
        compression_algorithm: NetworkSettingsCompressionAlgorithm::Deflate,
        client_throttle: false,
        client_throttle_threshold: 0,
        client_throttle_scalar: 0.0,
    };
    let (mut server, mut game) = (BatchCodec::default(), BatchCodec::default());
    let out = relay.on_server_message(&batch(&mut server, &[encode(&settings), encode(&text("hi"))])).unwrap();
    assert_eq!(out.to_game.len(), 2, "NetworkSettings leaves in its own uncompressed batch");
    assert!(decode(&mut game, &out.to_game[0])[0].is::<NetworkSettings>());
    for codec in [&mut server, &mut game] {
        codec.enable_compression(Algorithm::Deflate, 1);
    }
    assert_eq!(decode(&mut game, &out.to_game[1]).iter().map(message).collect::<Vec<_>>(), ["HI"]);

    let out = relay.on_game_message(&batch(&mut game, &[encode(&text("yo"))])).unwrap();
    let got = decode(&mut server, &out.to_server[0]);
    assert_eq!(got.iter().map(message).collect::<Vec<_>>(), ["yo", "[via proxy]"]);
}

#[test]
fn injections_wait_for_their_sides_handshake() {
    let mut relay = relay(Vec::new());
    let out = relay.inject([(Direction::ToServer, encode(&text("early"))), (Direction::ToGame, encode(&text("held")))]);
    assert!(out.to_server.is_empty() && out.to_game.is_empty());

    let out = relay.on_game_message(&batch(&mut BatchCodec::default(), &[encode(&ClientToServerHandshake {})])).unwrap();
    assert_eq!(out.to_server.len(), 2, "the handshake, then the held injection in its own batch");
    assert!(decode(&mut BatchCodec::default(), &out.to_server[0])[0].is::<ClientToServerHandshake>());
    assert_eq!(decode(&mut BatchCodec::default(), &out.to_server[1]).iter().map(message).collect::<Vec<_>>(), ["early"]);
    assert!(out.to_game.is_empty(), "the game side has had no handshake yet");

    let out = relay.inject([(Direction::ToServer, encode(&text("later")))]);
    assert_eq!(decode(&mut BatchCodec::default(), &out.to_server[0]).iter().map(message).collect::<Vec<_>>(), ["later"]);
}
