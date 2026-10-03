//! Login packet connection request: `i32le len ‖ envelope JSON ‖ i32le len ‖ client-data JWT`.
//! The vanilla 1.26.52 client sends a token-only envelope, `{"AuthenticationType":0,"Token":".."}`
//! (docs/research/vanilla-capture-2026-10-01.md); the legacy `Certificate` chain form remains for
//! offline logins and for credentials without a multiplayer token.

use p384::ecdsa::SigningKey;
use serde::Serialize;

use super::ClientData;
use crate::jwt::{self, now_unix, public_key_der_b64};
use crate::LoginCredentials;

/// Audience of multiplayer tokens (real and self-signed).
pub const MULTIPLAYER_AUDIENCE: &str = "api://auth-minecraft-services/multiplayer";

/// `AuthenticationType` values: 0 = full (Xbox), 1 = guest (rejected by servers), 2 = self-signed.
const AUTH_TYPE_FULL: u8 = 0;
const AUTH_TYPE_SELF_SIGNED: u8 = 2;

const SIX_HOURS: i64 = 6 * 3600;

#[derive(Serialize)]
struct Envelope<'a> {
    #[serde(rename = "Certificate")]
    certificate: String,
    #[serde(rename = "AuthenticationType")]
    authentication_type: u8,
    #[serde(rename = "Token")]
    token: &'a str,
}

/// What the vanilla client sends: no legacy chain, just the multiplayer token.
#[derive(Serialize)]
struct TokenEnvelope<'a> {
    #[serde(rename = "AuthenticationType")]
    authentication_type: u8,
    #[serde(rename = "Token")]
    token: &'a str,
}

#[derive(Serialize)]
struct Certificate<'a> {
    chain: &'a [String],
}

#[derive(Serialize)]
struct HeadClaims<'a> {
    exp: i64,
    nbf: i64,
    #[serde(rename = "identityPublicKey")]
    identity_public_key: &'a str,
    #[serde(rename = "certificateAuthority")]
    certificate_authority: bool,
}

/// Claims of a self-signed multiplayer token (gophertunnel `login.tokenClaims`).
#[derive(Serialize)]
struct SelfSignedTokenClaims<'a> {
    aud: &'static str,
    exp: i64,
    nbf: i64,
    ipt: &'static str,
    mid: &'a str,
    tid: &'a str,
    cpk: &'a str,
    xid: &'a str,
    xname: &'a str,
    leguuid: &'a str,
}

/// Online connection request: token-only, as the vanilla client sends it. Without a multiplayer
/// token, falls back to the legacy form: a self-signed head JWT (x5u = client key,
/// `identityPublicKey` = the Mojang chain's first x5u) prepended to the Mojang chain. `client` is
/// usually a [`ClientData`]; acacia-mitm passes the game's own claims.
pub fn build_connection_request(
    creds: &LoginCredentials,
    key: &SigningKey,
    client: &impl Serialize,
) -> Vec<u8> {
    if let Some(token) = creds.multiplayer_token.as_deref() {
        let envelope = TokenEnvelope { authentication_type: AUTH_TYPE_FULL, token };
        return frame(&serde_json::to_vec(&envelope).expect("serializable"), &sign_client(key, client));
    }
    let mut chain = Vec::with_capacity(creds.chain.len() + 1);
    let mojang_x5u = creds
        .chain
        .first()
        .and_then(|first| jwt::decode(first).ok())
        .and_then(|t| t.x5u().map(str::to_owned));
    match mojang_x5u {
        Some(x5u) => {
            let now = now_unix();
            chain.push(sign(key, &HeadClaims {
                exp: now + SIX_HOURS,
                nbf: now - SIX_HOURS,
                identity_public_key: &x5u,
                certificate_authority: true,
            }));
            chain.extend(creds.chain.iter().cloned());
        }
        None => chain.push(String::new()),
    }
    encode(&chain, AUTH_TYPE_FULL, "", &sign_client(key, client))
}

/// Connection request for `online-mode=false` servers: dummy chain `[""]` plus a self-signed
/// multiplayer token, as gophertunnel `login.EncodeOffline(.., legacy=false)` builds it for
/// 1.26.10+ servers. The identity UUID is derived from the name so it is stable across joins.
pub fn build_offline_connection_request(
    display_name: &str,
    key: &SigningKey,
    client: &ClientData,
) -> Vec<u8> {
    let uuid = offline_identity(display_name);
    build_offline_connection_request_for(&OfflineIdentity { name: display_name, xuid: "", uuid: &uuid }, key, client)
}

/// Who a self-signed multiplayer token names.
pub struct OfflineIdentity<'a> {
    pub name: &'a str,
    pub xuid: &'a str,
    pub uuid: &'a str,
}

/// [`build_offline_connection_request`] for any identity and client-data claims (acacia-mitm
/// re-signs the game's own).
pub fn build_offline_connection_request_for(
    identity: &OfflineIdentity,
    key: &SigningKey,
    client: &impl Serialize,
) -> Vec<u8> {
    let now = now_unix();
    let cpk = public_key_der_b64(key);
    let token = sign(key, &SelfSignedTokenClaims {
        aud: MULTIPLAYER_AUDIENCE,
        exp: now + SIX_HOURS,
        nbf: now - SIX_HOURS,
        ipt: "",
        mid: "",
        tid: "",
        cpk: &cpk,
        xid: identity.xuid,
        xname: identity.name,
        leguuid: identity.uuid,
    });
    encode(&[String::new()], AUTH_TYPE_SELF_SIGNED, &token, &sign_client(key, client))
}

/// Splits a connection request into its envelope JSON and client-data JWT.
pub fn split_connection_request(request: &[u8]) -> Option<(&[u8], &[u8])> {
    let (envelope, rest) = take_prefixed(request)?;
    let (client_jwt, _) = take_prefixed(rest)?;
    Some((envelope, client_jwt))
}

fn take_prefixed(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = usize::try_from(i32::from_le_bytes(buf.get(..4)?.try_into().ok()?)).ok()?;
    let rest = &buf[4..];
    (rest.len() >= len).then(|| rest.split_at(len))
}

/// MD5 name-based UUID (version 3 bits) of `OfflinePlayer:<name>`.
pub fn offline_identity(display_name: &str) -> String {
    use md5::{Digest, Md5};
    let mut id: [u8; 16] = Md5::digest(format!("OfflinePlayer:{display_name}")).into();
    id[6] = (id[6] & 0x0f) | 0x30;
    id[8] = (id[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(id).to_string()
}

fn sign(key: &SigningKey, claims: &impl Serialize) -> String {
    jwt::sign(key, claims).expect("login claims always serialize")
}

/// The client-data JWT with claims in byte order of their names, as the vanilla client sorts them.
/// Sorted explicitly: serde_json's map keeps insertion order if any crate enables `preserve_order`.
fn sign_client(key: &SigningKey, client: &impl Serialize) -> String {
    let claims = serde_json::to_value(client).expect("client data serializes");
    let sorted: std::collections::BTreeMap<String, serde_json::Value> =
        claims.as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.clone())).collect();
    sign(key, &sorted)
}

fn encode(chain: &[String], authentication_type: u8, token: &str, client_jwt: &str) -> Vec<u8> {
    let certificate = serde_json::to_string(&Certificate { chain }).expect("serializable");
    let envelope = serde_json::to_vec(&Envelope { certificate, authentication_type, token })
        .expect("serializable");
    frame(&envelope, client_jwt)
}

fn frame(envelope: &[u8], client_jwt: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + envelope.len() + client_jwt.len());
    push_prefixed(&mut out, envelope);
    push_prefixed(&mut out, client_jwt.as_bytes());
    out
}

fn push_prefixed(out: &mut Vec<u8>, data: &[u8]) {
    let len = i32::try_from(data.len()).expect("login payload under 2 GiB");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(data);
}
