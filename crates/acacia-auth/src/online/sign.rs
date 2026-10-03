//! Xbox Live request signing (go-xsapi `internal.SignaturePolicy.Generate`, policy version 1).
//!
//! `Signature` header = base64(`u32be(1) ‖ i64be(filetime) ‖ r ‖ s`), where the P-256/SHA-256
//! signature covers `u32be(1) 0 ‖ i64be(filetime) 0 ‖ method 0 ‖ path?query 0 ‖ Authorization 0
//! ‖ body 0`.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::{Value, json};

const POLICY_VERSION: u32 = 1;
/// 100ns intervals between 1601-01-01 and 1970-01-01.
const FILETIME_UNIX_EPOCH: i64 = 116_444_736_000_000_000;

/// Windows FILETIME for a Unix time in nanoseconds (go-xsapi `windowsTimestamp`).
pub fn filetime(unix_nanos: i64) -> i64 {
    unix_nanos / 100 + FILETIME_UNIX_EPOCH
}

/// The exact bytes whose SHA-256 is signed.
pub fn signing_input(
    filetime: i64,
    method: &str,
    path_and_query: &str,
    authorization: &str,
    body: &[u8],
) -> Vec<u8> {
    let mut m = Vec::with_capacity(32 + path_and_query.len() + authorization.len() + body.len());
    m.extend_from_slice(&POLICY_VERSION.to_be_bytes());
    m.push(0);
    m.extend_from_slice(&filetime.to_be_bytes());
    m.push(0);
    for part in [method.as_bytes(), path_and_query.as_bytes(), authorization.as_bytes(), body] {
        m.extend_from_slice(part);
        m.push(0);
    }
    m
}

/// Raw signature bytes (before base64): 12-byte prefix plus 64-byte `r ‖ s`.
pub fn signature_bytes(
    key: &SigningKey,
    filetime: i64,
    method: &str,
    path_and_query: &str,
    authorization: &str,
    body: &[u8],
) -> Vec<u8> {
    let sig: Signature = key.sign(&signing_input(filetime, method, path_and_query, authorization, body));
    let mut out = Vec::with_capacity(76);
    out.extend_from_slice(&POLICY_VERSION.to_be_bytes());
    out.extend_from_slice(&filetime.to_be_bytes());
    out.extend_from_slice(&sig.to_bytes());
    out
}

/// Value for the `Signature` header.
pub fn signature_header(
    key: &SigningKey,
    filetime: i64,
    method: &str,
    path_and_query: &str,
    authorization: &str,
    body: &[u8],
) -> String {
    STANDARD.encode(signature_bytes(key, filetime, method, path_and_query, authorization, body))
}

/// `ProofKey` JWK as go-jose marshals `JSONWebKey{Algorithm: ES256, Use: sig}`.
pub fn proof_key_jwk(key: &SigningKey) -> Value {
    let point = key.verifying_key().to_encoded_point(false);
    json!({
        "use": "sig",
        "kty": "EC",
        "crv": "P-256",
        "alg": "ES256",
        "x": URL_SAFE_NO_PAD.encode(point.x().expect("uncompressed point")),
        "y": URL_SAFE_NO_PAD.encode(point.y().expect("uncompressed point")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Verifier;
    use p256::ecdsa::VerifyingKey;
    use p256::pkcs8::DecodePublicKey;

    // Produced by the Go program in the scratchpad (gophertunnel v1.62 / go-xsapi v2.0.3
    // `nsal.AuthPolicy.Generate`) with the inputs below. Go signs with a random nonce, so we
    // check the prefix byte-for-byte and verify Go's r||s over our reconstructed input.
    const GO_PUBLIC_KEY_DER_B64: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAExmmbDxHTB7x26Fs8hx92wh87Doyk+lZt02Pumv/sz9F39lpvoJTfg9ZlSL4zrWfMg3i84ygQHKdTX1DwVBR7eQ==";
    const GO_SIGNATURE_B64: &str = "AAAAAQHdSdNazBaHxD30sfs+6d6aLkqbHuuwmV4BGHH8IgJ9JhwArKvdZv+hqETYsdfddcjCK+IFB1JJS75vvM/gWkH6almIYfsdng==";
    const UNIX_NANOS: i64 = 1_790_000_000_123_456_700;
    const PATH: &str = "/xsts/authorize?x=1";
    const AUTHORIZATION: &str = "XBL3.0 x=123;abc";
    const BODY: &[u8] = b"{\"RelyingParty\":\"http://xboxlive.com\"}\n";

    #[test]
    fn matches_go_xsapi_signature() {
        let sig = STANDARD.decode(GO_SIGNATURE_B64).unwrap();
        let ft = filetime(UNIX_NANOS);
        assert_eq!(sig.len(), 76);
        assert_eq!(&sig[..4], &1u32.to_be_bytes());
        assert_eq!(&sig[4..12], &ft.to_be_bytes());

        let der = STANDARD.decode(GO_PUBLIC_KEY_DER_B64).unwrap();
        let go_key = VerifyingKey::from_public_key_der(&der).unwrap();
        let rs = Signature::from_slice(&sig[12..]).unwrap();
        go_key
            .verify(&signing_input(ft, "POST", PATH, AUTHORIZATION, BODY), &rs)
            .expect("Go signature must verify over the Rust signing input");
    }

    #[test]
    fn own_signature_verifies() {
        let key = SigningKey::random(&mut rand_core::OsRng);
        let ft = filetime(UNIX_NANOS);
        let raw = signature_bytes(&key, ft, "POST", PATH, "", BODY);
        let rs = Signature::from_slice(&raw[12..]).unwrap();
        key.verifying_key()
            .verify(&signing_input(ft, "POST", PATH, "", BODY), &rs)
            .unwrap();
    }
}
