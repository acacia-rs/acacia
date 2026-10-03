//! Test stand-ins for the authorization service (RS256 issuer), Mojang's chain and a client.

use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p384::ecdsa::SigningKey;
use rand_core::OsRng;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey as RsaSigner;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::traits::PublicKeyParts;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::Sha256;

use super::{MULTIPLAYER_ISSUER, SigningKeys, Verifier};
use crate::jwt::{self, public_key_der_b64};
use crate::login::{ClientData, MULTIPLAYER_AUDIENCE, build_connection_request};
use crate::LoginCredentials;

pub const NOW: i64 = 1_800_000_000;
pub const KID: &str = "TESTKID";
pub const XUID: &str = "2535416196521234";

/// One 2048-bit key for the whole test binary: generating it dominates the run time.
fn issuer_key() -> &'static RsaPrivateKey {
    static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
    KEY.get_or_init(|| RsaPrivateKey::new(&mut OsRng, 2048).expect("RSA key generation"))
}

pub fn jwks() -> String {
    let key = issuer_key();
    let b64 = |n: &rsa::BigUint| URL_SAFE_NO_PAD.encode(n.to_bytes_be());
    // x5t in hex, as the real service sends it.
    json!({"keys": [{"kty": "RSA", "use": "sig", "kid": KID, "x5t": "472E53B520276E1F3B6BA6F95EBF09D61C2568BE", "n": b64(key.n()), "e": b64(key.e())}]})
        .to_string()
}

pub fn verifier() -> Verifier {
    Verifier::new(SigningKeys::from_jwks(&jwks()).unwrap())
}

pub fn ec_key() -> SigningKey {
    SigningKey::random(&mut OsRng)
}

pub fn public(key: &SigningKey) -> p384::PublicKey {
    key.verifying_key().into()
}

/// A JWT with any header, carrying the issuer's RS256 signature.
pub fn issuer_jwt(header: &Value, claims: &Value) -> String {
    let input = format!("{}.{}", URL_SAFE_NO_PAD.encode(header.to_string()), URL_SAFE_NO_PAD.encode(claims.to_string()));
    let signature = RsaSigner::<Sha256>::new(issuer_key().clone()).sign(input.as_bytes()).to_vec();
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
}

pub fn rs256(claims: &Value) -> String {
    issuer_jwt(&json!({"alg": "RS256", "kid": KID, "typ": "JWT"}), claims)
}

/// What the authorization service puts in a multiplayer token for `client`.
pub fn token_claims(client: &SigningKey) -> Value {
    json!({
        "iss": MULTIPLAYER_ISSUER, "aud": MULTIPLAYER_AUDIENCE, "exp": NOW + 3600, "nbf": NOW - 60,
        "ipt": "PlayFab", "mid": "A1B2C3D4E5F60718", "tid": "20CA2",
        "cpk": public_key_der_b64(client), "xid": XUID, "xname": "Steve",
    })
}

pub fn client_data() -> ClientData {
    ClientData::default_for("Steve", "play.example.org:19132", "1.26.52")
}

pub fn credentials(chain: Vec<String>, token: Option<String>) -> LoginCredentials {
    LoginCredentials {
        chain,
        multiplayer_token: token,
        xuid: XUID.into(),
        display_name: "Steve".into(),
        identity: String::new(),
        playfab_id: None,
        expires_at: 0,
    }
}

/// A token login built by acacia's own client code.
pub fn token_request(token: String, client: &SigningKey) -> Vec<u8> {
    build_connection_request(&credentials(Vec::new(), Some(token)), client, &client_data())
}

/// A connection request from raw parts.
pub fn raw_request(envelope: &[u8], client_jwt: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for part in [envelope, client_jwt] {
        out.extend_from_slice(&(part.len() as i32).to_le_bytes());
        out.extend_from_slice(part);
    }
    out
}

pub fn client_jwt(key: &SigningKey, claims: &impl Serialize) -> String {
    jwt::sign(key, claims).unwrap()
}

pub fn extra_data(xuid: &str) -> Value {
    json!({"XUID": xuid, "identity": "8a5f1c9e-3b7d-3f21-9c4e-6d2b8f0a1e57", "displayName": "Steve", "titleId": "896928775"})
}

/// The two JWTs `multiplayer.minecraft.net/authentication` returns for `client`, signed by `root`
/// instead of Mojang.
pub fn mojang_chain(root: &SigningKey, client: &SigningKey, xuid: &str) -> Vec<String> {
    let intermediate = ec_key();
    let validity = json!({"iss": "Mojang", "exp": jwt::now_unix() + 3600, "nbf": jwt::now_unix() - 60});
    let with = |extra: Value| {
        let mut claims = validity.clone();
        claims.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        claims
    };
    vec![
        client_jwt(root, &with(json!({"identityPublicKey": public_key_der_b64(&intermediate), "certificateAuthority": true}))),
        client_jwt(&intermediate, &with(json!({"identityPublicKey": public_key_der_b64(client), "extraData": extra_data(xuid)}))),
    ]
}
