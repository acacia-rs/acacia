use acacia_auth::login::{build_offline_connection_request, parse_server_handshake, ClientData, SigningKeys};
use acacia_proto::packets::ServerToClientHandshake;
use p384::ecdsa::SigningKey;

use super::*;
use crate::batch::BatchCodec;
use crate::compression::Algorithm;
use crate::crypto::derive_key;

const NOW: i64 = 1_800_000_000;

/// Offline logins are signed with the real clock.
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

fn key(seed: u8) -> SigningKey {
    SigningKey::from_slice(&[seed; 48]).unwrap()
}

fn server(require_authenticated: bool) -> ServerLogin {
    ServerLogin::new(ServerConnection::new(key(1)), LoginConfig { require_authenticated, ..LoginConfig::default() })
}

fn verifier() -> Verifier {
    Verifier::new(SigningKeys::empty())
}

fn login_packet(protocol: i32, client: &SigningKey) -> Bytes {
    let data = ClientData::default_for("Bot_1", "127.0.0.1:19132", acacia_proto::GAME_VERSION);
    let request = build_offline_connection_request("Bot_1", client, &data);
    let mut buf = BytesMut::new();
    codec::write_varint(&mut buf, Login::ID);
    codec::write_i32(&mut buf, protocol);
    codec::write_varint(&mut buf, request.len() as u32);
    buf.extend_from_slice(&request);
    buf.freeze()
}

/// The client's side of the wire: sends packets, returns the packets in the server's replies.
struct Wire(BatchCodec);

impl Wire {
    fn send(&mut self, login: &mut ServerLogin, packet: Bytes, now: i64) -> Result<(Step, Vec<RawPacket>), Rejected> {
        let step = login.handle(&self.0.encode([&packet[..]]), &verifier(), now)?;
        let packets = self.read(&step.send);
        Ok((step, packets))
    }

    fn read(&mut self, messages: &[Bytes]) -> Vec<RawPacket> {
        let mut out = Vec::new();
        messages.iter().for_each(|m| self.0.decode(m, &mut out).unwrap());
        out.into_iter().map(|b| RawPacket::parse(b).unwrap()).collect()
    }
}

#[test]
fn offline_login_reaches_login_success_over_compression_and_encryption() {
    let (client, mut login, mut wire) = (key(2), server(false), Wire(BatchCodec::default()));
    let now = now();

    let (step, replies) = wire.send(&mut login, encode(&RequestNetworkSettings { client_protocol: PROTOCOL_VERSION }), now).unwrap();
    assert!(step.done.is_none());
    let settings: NetworkSettings = replies[0].decode().unwrap();
    wire.0.enable_compression(Algorithm::from_settings(settings.compression_algorithm).unwrap(), settings.compression_threshold.into());

    let (step, replies) = wire.send(&mut login, login_packet(PROTOCOL_VERSION, &client), now).unwrap();
    assert!(step.done.is_none());
    let handshake: ServerToClientHandshake = replies[0].decode().unwrap();
    let (server_key, salt) = parse_server_handshake(&handshake.token).unwrap();
    wire.0.enable_encryption(derive_key(&client, &server_key, &salt));

    let (step, replies) = wire.send(&mut login, encode(&ClientToServerHandshake {}), now).unwrap();
    assert_eq!(replies[0].decode::<PlayStatus>().unwrap().status, PlayStatusStatus::LoginSuccess);
    let done = step.done.expect("login completes on the client's handshake");
    assert_eq!((done.identity.display_name.as_str(), done.authenticated), ("Bot_1", false));

    // The connection carries on encrypted and compressed.
    let mut conn = login.into_connection();
    let later = conn.encode(&[encode(&PlayStatus { status: PlayStatusStatus::PlayerSpawn })]);
    assert_eq!(wire.read(&[later])[0].decode::<PlayStatus>().unwrap().status, PlayStatusStatus::PlayerSpawn);
}

#[test]
fn online_mode_rejects_an_offline_login_with_a_disconnect() {
    let (client, mut login, mut wire) = (key(2), server(true), Wire(BatchCodec::default()));
    let now = now();
    wire.send(&mut login, encode(&RequestNetworkSettings { client_protocol: PROTOCOL_VERSION }), now).unwrap();
    wire.0.enable_compression(Algorithm::Deflate, 1);

    let rejected = wire.send(&mut login, login_packet(PROTOCOL_VERSION, &client), now).unwrap_err();
    assert!(matches!(rejected.error, LoginError::NotAuthenticated));
    let notice = wire.read(&[rejected.send.unwrap()]);
    assert_eq!(notice[0].decode::<Disconnect>().unwrap().reason, DisconnectFailReason::NotAuthenticated);
}

#[test]
fn wrong_protocol_is_told_which_side_is_outdated() {
    for (client, status) in [(PROTOCOL_VERSION - 1, PlayStatusStatus::FailedClient), (PROTOCOL_VERSION + 1, PlayStatusStatus::FailedSpawn)] {
        let (mut login, mut wire) = (server(false), Wire(BatchCodec::default()));
        let rejected = wire.send(&mut login, encode(&RequestNetworkSettings { client_protocol: client }), NOW).unwrap_err();
        assert!(matches!(rejected.error, LoginError::Protocol { .. }));
        assert_eq!(wire.read(&[rejected.send.unwrap()])[0].decode::<PlayStatus>().unwrap().status, status);
    }
}

#[test]
fn out_of_order_and_garbage_are_rejected_without_panicking() {
    let (mut login, mut wire) = (server(false), Wire(BatchCodec::default()));
    let rejected = wire.send(&mut login, login_packet(PROTOCOL_VERSION, &key(2)), NOW).unwrap_err();
    assert!(matches!(rejected.error, LoginError::Unexpected(id) if id == Login::ID));

    let mut login = server(false);
    assert!(login.handle(&[0xfe, 0xff, 0xff, 0xff], &verifier(), NOW).is_err());
    assert!(login.handle(&[], &verifier(), NOW).is_err());
}
