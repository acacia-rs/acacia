use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use p384::ecdsa::SigningKey;
use rand_core::{OsRng, RngCore};

use super::split_connection_request;
use crate::jwt;
use crate::{Error, Result};

/// Parses the ServerToClientHandshake JWT: verifies its ES384 signature against its own `x5u`
/// key and returns `(server_public_key, salt)`.
pub fn parse_server_handshake(token: &str) -> Result<(p384::PublicKey, Vec<u8>)> {
    let (key, claims) = jwt::decode_self_signed(token)?;
    let salt = claims
        .get("salt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Jwt("handshake JWT has no salt claim".into()))?;
    let salt = STANDARD
        .decode(salt)
        .or_else(|_| STANDARD_NO_PAD.decode(salt))
        .map_err(|e| Error::Jwt(format!("salt base64: {e}")))?;
    Ok((key, salt))
}

/// The server side: a ServerToClientHandshake token signed by `key` with a fresh salt.
pub fn build_server_handshake(key: &SigningKey) -> (String, [u8; 16]) {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let token = jwt::sign(key, &serde_json::json!({ "salt": STANDARD.encode(salt) })).expect("handshake claims serialize");
    (token, salt)
}

/// The server side: the client's key (its client-data JWT's `x5u`) from a connection request.
pub fn client_public_key(request: &[u8]) -> Result<p384::PublicKey> {
    let (_, client_jwt) = split_connection_request(request).ok_or_else(|| Error::Jwt("malformed connection request".into()))?;
    let client_jwt = std::str::from_utf8(client_jwt).map_err(|e| Error::Jwt(e.to_string()))?;
    let token = jwt::decode(client_jwt)?;
    jwt::parse_public_key_der_b64(token.x5u().ok_or_else(|| Error::Jwt("client data has no x5u".into()))?)
}
