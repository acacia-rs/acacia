use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use p384::ecdsa::SigningKey;
use rand_core::OsRng;
use serde_json::{Value, json};

use super::*;
use crate::jwt::{self, public_key_der_b64};
use crate::LoginCredentials;

fn key() -> SigningKey {
    SigningKey::random(&mut OsRng)
}

fn pubkey(k: &SigningKey) -> p384::PublicKey {
    k.verifying_key().into()
}

/// Splits a connection request into (envelope JSON, client-data JWT), asserting exact framing.
fn split(req: &[u8]) -> (Value, String) {
    let len = i32::from_le_bytes(req[..4].try_into().unwrap()) as usize;
    let envelope = serde_json::from_slice(&req[4..4 + len]).unwrap();
    let rest = &req[4 + len..];
    let len2 = i32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
    assert_eq!(rest.len(), 4 + len2, "no trailing bytes");
    (envelope, String::from_utf8(rest[4..].to_vec()).unwrap())
}

fn chain_of(envelope: &Value) -> Vec<String> {
    let cert: Value = serde_json::from_str(envelope["Certificate"].as_str().unwrap()).unwrap();
    serde_json::from_value(cert["chain"].clone()).unwrap()
}

#[test]
fn jwt_round_trip_and_tamper() {
    let k = key();
    let token = jwt::sign(&k, &json!({"a": 1})).unwrap();
    let (pk, claims) = jwt::decode_self_signed(&token).unwrap();
    assert_eq!(pk, pubkey(&k));
    assert_eq!(claims, json!({"a": 1}));
    let sig_len = jwt::decode(&token).map(|_| token.rsplit('.').next().unwrap().len()).unwrap();
    assert_eq!(sig_len, 128, "96-byte raw r||s is 128 base64url chars");

    let other = jwt::sign(&key(), &json!({"a": 2})).unwrap();
    let forged = format!(
        "{}.{}",
        token.rsplit_once('.').unwrap().0,
        other.rsplit_once('.').unwrap().1
    );
    assert!(jwt::decode_self_signed(&forged).is_err());
}

#[test]
fn public_key_round_trip() {
    let k = key();
    let b64 = public_key_der_b64(&k);
    assert_eq!(STANDARD.decode(&b64).unwrap().len(), 120, "P-384 SPKI DER is 120 bytes");
    assert_eq!(jwt::parse_public_key_der_b64(&b64).unwrap(), pubkey(&k));
}

#[test]
fn offline_request_structure() {
    let k = key();
    let client = ClientData::default_for("Bot_1", "127.0.0.1:19132", "1.26.50");
    let (env, client_jwt) = split(&build_offline_connection_request("Bot_1", &k, &client));

    assert_eq!(env["AuthenticationType"], 2);
    assert_eq!(chain_of(&env), vec![String::new()]);
    let (pk, token) = jwt::decode_self_signed(env["Token"].as_str().unwrap()).unwrap();
    assert_eq!(pk, pubkey(&k));
    assert_eq!(token["aud"], MULTIPLAYER_AUDIENCE);
    assert_eq!(token["cpk"], public_key_der_b64(&k));
    assert_eq!(token["xname"], "Bot_1");
    assert_eq!(token["xid"], "");
    assert_eq!(token["leguuid"], offline_identity("Bot_1"));
    assert!(token["exp"].as_i64().unwrap() > token["nbf"].as_i64().unwrap());

    let (_, cd) = jwt::decode_self_signed(&client_jwt).unwrap();
    let decoded: ClientData = serde_json::from_value(cd).unwrap();
    assert_eq!(decoded, client);
    let skin = STANDARD.decode(&decoded.skin.skin_data).unwrap();
    assert_eq!(skin.len() as u32, decoded.skin.skin_image_width * decoded.skin.skin_image_height * 4);
}

#[test]
fn client_data_has_exactly_vanilla_keys() {
    let client = ClientData::default_for("Bot_1", "127.0.0.1:19132", "1.26.50");
    let (_, client_jwt) = split(&build_offline_connection_request("Bot_1", &key(), &client));
    let (_, cd) = jwt::decode_self_signed(&client_jwt).unwrap();
    let mut keys: Vec<&str> = cd.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, VANILLA_CLIENT_DATA_KEYS);
}

/// The client-data claim names the vanilla Windows 1.26.52 client sent, sorted
/// (docs/research/vanilla-capture-2026-10-01.md).
const VANILLA_CLIENT_DATA_KEYS: [&str; 46] = [
    "AnimatedImageData", "ArmSize", "CapeData", "CapeId", "CapeImageHeight", "CapeImageWidth",
    "CapeOnClassicSkin", "ClientEditorConnectionIntent", "ClientIsEditorCapable", "ClientRandomId",
    "CompatibleWithClientSideChunkGen", "CurrentInputMode", "DefaultInputMode", "DeviceId",
    "DeviceModel", "DeviceOS", "FilterProfanity", "GameVersion", "GraphicsMode", "GuiScale",
    "LanguageCode", "MaxViewDistance", "MemoryTier", "OverrideSkin", "PersonaPieces", "PersonaSkin",
    "PieceTintColors", "PlatformOfflineId", "PlatformOnlineId", "PlatformType", "PremiumSkin",
    "ProfileHash", "SelfSignedId", "ServerAddress", "SkinAnimationData", "SkinColor", "SkinData",
    "SkinGeometryData", "SkinGeometryDataEngineVersion", "SkinId", "SkinImageHeight",
    "SkinImageWidth", "SkinResourcePatch", "ThirdPartyName", "TrustedSkin", "UIProfile",
];

fn online_creds(mojang: &SigningKey, identity: &SigningKey, token: Option<&str>) -> LoginCredentials {
    // Stand-in for multiplayer.minecraft.net's 2-JWT chain.
    let chain = vec![
        jwt::sign(mojang, &json!({"identityPublicKey": public_key_der_b64(identity)})).unwrap(),
        jwt::sign(identity, &json!({"extraData": {"XUID": "123"}})).unwrap(),
    ];
    LoginCredentials {
        chain,
        multiplayer_token: token.map(str::to_owned),
        xuid: "123".into(),
        display_name: "Steve".into(),
        identity: "00000000-0000-3000-8000-000000000000".into(),
        playfab_id: None,
        expires_at: 0,
    }
}

#[test]
fn online_request_is_token_only_with_sorted_client_data() {
    let client_key = key();
    let creds = online_creds(&key(), &key(), Some("mp.token.value"));
    let client = ClientData::default_for("Steve", "example.org:19132", "1.26.50");
    let req = build_connection_request(&creds, &client_key, &client);
    let len = i32::from_le_bytes(req[..4].try_into().unwrap()) as usize;
    assert_eq!(&req[4..4 + len], br#"{"AuthenticationType":0,"Token":"mp.token.value"}"#);

    let (_, client_jwt) = split(&req);
    let (_, claims) = jwt::decode_self_signed(&client_jwt).unwrap();
    assert!(claims.get("PlayFabId").is_none() && claims.get("PartyId").is_none());
    // Order on the wire: each claim name appears in the raw payload after the previous one.
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(client_jwt.split('.').nth(1).unwrap()).unwrap();
    let raw = String::from_utf8(raw).unwrap();
    let mut names: Vec<&String> = claims.as_object().unwrap().keys().collect();
    names.sort_unstable();
    let positions: Vec<usize> = names.iter().map(|k| raw.find(&format!("\"{k}\":")).unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "claims not in sorted order: {raw:.200}");
}

#[test]
fn online_request_without_token_prepends_self_signed_head() {
    let (mojang, identity, client_key) = (key(), key(), key());
    let creds = online_creds(&mojang, &identity, None);
    let chain = creds.chain.clone();
    let client = ClientData::default_for("Steve", "example.org:19132", "1.26.50");
    let (env, client_jwt) = split(&build_connection_request(&creds, &client_key, &client));

    assert_eq!(env["AuthenticationType"], 0);
    let sent = chain_of(&env);
    assert_eq!(sent.len(), 3);
    assert_eq!(&sent[1..], &chain[..]);
    let (head_key, head) = jwt::decode_self_signed(&sent[0]).unwrap();
    assert_eq!(head_key, pubkey(&client_key));
    assert_eq!(head["identityPublicKey"], public_key_der_b64(&mojang));
    assert_eq!(head["certificateAuthority"], true);
    assert!(jwt::decode_self_signed(&client_jwt).is_ok());
}

#[test]
fn server_handshake() {
    let server = key();
    let salt = [7u8; 16];
    let token = jwt::sign(&server, &json!({"salt": STANDARD.encode(salt)})).unwrap();
    let (pk, got) = parse_server_handshake(&token).unwrap();
    assert_eq!(pk, pubkey(&server));
    assert_eq!(got, salt);

    let mut bad = token.clone();
    bad.replace_range(bad.len() - 4.., "AAAA");
    assert!(parse_server_handshake(&bad).is_err());
}
