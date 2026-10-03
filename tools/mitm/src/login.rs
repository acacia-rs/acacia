//! The game's Login: summarised for the capture without secrets, and re-signed with the proxy's key
//! for the server (an offline login carrying the game's identity and client data verbatim).

use std::fmt;

use base64::Engine;
use acacia_auth::jwt;
use acacia_auth::login::{OfflineIdentity, build_offline_connection_request_for, offline_identity, split_connection_request};
use acacia_proto::packets::Login;
use acacia_proto::{Packet, codec};
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};

/// Strings longer than this (skin images, geometry) are logged as their size.
const MAX_LOGGED_STRING: usize = 256;

pub struct GameLogin {
    /// Login for the server, signed with the proxy's key.
    pub upstream: Bytes,
    /// The game's key (the client-data JWT's x5u), for the game-side ECDH.
    pub game_key: p384::PublicKey,
    pub client_data: Value,
    /// Structure only: JWT headers, claim names and kept non-secret values. Never token bodies.
    pub summary: Value,
    pub identity: Value,
}

pub fn read(body: &[u8], key: &SigningKey) -> Result<GameLogin, String> {
    let mut r = body;
    let protocol = codec::read_i32(&mut r).map_err(|e| e.to_string())?;
    let len = codec::read_varint(&mut r).map_err(|e| e.to_string())? as usize;
    let request = r.get(..len).ok_or("Login tokens truncated")?;
    let (envelope, client_jwt) = split_connection_request(request).ok_or("malformed connection request")?;
    let client_jwt = std::str::from_utf8(client_jwt).map_err(|e| e.to_string())?;
    let client = jwt::decode(client_jwt).map_err(|e| e.to_string())?;
    let game_key = acacia_auth::login::client_public_key(request).map_err(|e| e.to_string())?;
    let outer: Value = serde_json::from_slice(envelope).map_err(|e| e.to_string())?;

    let token_claims = outer["Token"].as_str().and_then(|t| jwt::decode(t).ok()).map(|t| t.claims);
    let chain_extra = chain(&outer).iter().filter_map(|j| jwt::decode(j).ok()).find_map(|j| j.claims.get("extraData").cloned());
    let claim = |token_key: &str, chain_key: &str| {
        let from_token = token_claims.as_ref().and_then(|c| c[token_key].as_str());
        from_token.or_else(|| chain_extra.as_ref().and_then(|e| e[chain_key].as_str())).filter(|s| !s.is_empty()).map(str::to_owned)
    };
    let name = claim("xname", "displayName").or_else(|| client.claims["ThirdPartyName"].as_str().map(str::to_owned)).ok_or("no player name")?;
    let xuid = claim("xid", "XUID").unwrap_or_default();
    let uuid = claim("leguuid", "identity").unwrap_or_else(|| offline_identity(&name));

    let request = build_offline_connection_request_for(&OfflineIdentity { name: &name, xuid: &xuid, uuid: &uuid }, key, &client.claims);
    let mut upstream = BytesMut::with_capacity(request.len() + 16);
    codec::write_varint(&mut upstream, Login::ID);
    codec::write_i32(&mut upstream, protocol);
    codec::write_varint(&mut upstream, request.len() as u32);
    upstream.extend_from_slice(&request);

    let summary = json!({
        "protocol": protocol,
        "chain_outer_keys": key_order(envelope),
        "chain": chain(&outer).iter().filter(|j| !j.is_empty()).map(|j| describe(j, &["certificateAuthority", "titleId"])).collect::<Vec<_>>(),
        "token": outer["Token"].as_str().filter(|t| !t.is_empty()).map(|t| describe(t, &[])),
        "auth_type": outer["AuthenticationType"],
        "client_data": describe(client_jwt, &[]),
        "client_data_values": trimmed(&client.claims),
    });
    let identity = json!({ "DisplayName": name, "Identity": uuid, "XUID": xuid });
    Ok(GameLogin { upstream: upstream.freeze(), game_key, client_data: client.claims, summary, identity })
}

/// The legacy `Certificate` chain, if the envelope has one.
fn chain(outer: &Value) -> Vec<String> {
    let cert = outer["Certificate"].as_str().and_then(|c| serde_json::from_str::<Value>(c).ok());
    cert.and_then(|c| serde_json::from_value(c["chain"].clone()).ok()).unwrap_or_default()
}

/// A JWT's header verbatim, its claim names in order, and only the `keep` claim values (searched one level deep).
fn describe(token: &str, keep: &[&str]) -> Value {
    let parts: Vec<&str> = token.split('.').collect();
    let Ok(decoded) = jwt::decode(token) else { return json!({ "malformed": true }) };
    let claims_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]).unwrap_or_default();
    let mut kept = serde_json::Map::new();
    for &k in keep {
        let nested = decoded.claims.as_object().into_iter().flat_map(|m| m.values()).find_map(|v| v.get(k));
        if let Some(v) = decoded.claims.get(k).or(nested) {
            kept.insert(k.to_owned(), v.clone());
        }
    }
    let mut out = json!({ "header": decoded.header.to_string(), "claim_order": key_order(&claims_json), "signature_len": parts[2].len() });
    if !kept.is_empty() {
        out["kept"] = Value::Object(kept);
    }
    out
}

/// `v` with long strings (images, geometry) replaced by their size.
pub fn trimmed(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), trimmed(v))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(trimmed).collect()),
        Value::String(s) if s.len() > MAX_LOGGED_STRING => Value::String(format!("<{} chars>", s.len())),
        other => other.clone(),
    }
}

/// An object's top-level keys in their original order (serde_json's map sorts them).
fn key_order(json: &[u8]) -> Vec<String> {
    struct Keys;
    impl<'de> Visitor<'de> for Keys {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut keys = Vec::new();
            while let Some((k, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                keys.push(k);
            }
            Ok(keys)
        }
    }
    let mut de = serde_json::Deserializer::from_slice(json);
    serde::Deserializer::deserialize_map(&mut de, Keys).unwrap_or_default()
}

/// The game-side ServerToClientHandshake (signed by the proxy) and the session key it sets up.
pub fn handshake_for_game(key: &SigningKey, game_key: &p384::PublicKey) -> (Bytes, [u8; 32]) {
    let (token, salt) = acacia_auth::login::build_server_handshake(key);
    let mut packet = BytesMut::new();
    acacia_proto::encode_packet(&acacia_proto::packets::ServerToClientHandshake { token }, &mut packet);
    (packet.freeze(), acacia_session::crypto::derive_key(key, game_key, &salt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_order_keeps_the_original_order() {
        assert_eq!(key_order(br#"{"b":1,"a":{"z":2},"c":[3]}"#), ["b", "a", "c"]);
    }

    #[test]
    fn trimmed_hides_long_strings() {
        let v = json!({ "SkinData": "x".repeat(300), "SkinId": "id", "AnimatedImageData": [{ "Image": "y".repeat(400) }] });
        let want = json!({ "SkinData": "<300 chars>", "SkinId": "id", "AnimatedImageData": [{ "Image": "<400 chars>" }] });
        assert_eq!(trimmed(&v), want);
    }
}
