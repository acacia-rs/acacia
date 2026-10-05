//! The `a=identity` every answer carries: a JWT self-signed by its `cpk` claim and a detached JWS
//! by that key over the answer's fingerprints. Verifying it proves the server holds the key and
//! owns the DTLS certificate; trust in the key itself is the caller's (pin it).

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p384::ecdsa::signature::Signer;
use p384::ecdsa::{Signature, SigningKey, VerifyingKey};
use p384::pkcs8::EncodePublicKey;
use serde_json::json;

use crate::assertion::{self, parse_cpk, split_jwt, verify_es384, verify_fingerprints, Reason};
use crate::identity::{identity_line, insert_session_line, Identity};
use crate::Error;

/// Checks the answer's assertion and returns the server's key (`cpk`).
pub(crate) fn verify_answer(sdp: &str, now_unix: i64) -> Result<VerifyingKey, Error> {
    check(sdp, now_unix).map_err(|reason| Error::ServerIdentity(reason.to_owned()))
}

fn check(sdp: &str, now_unix: i64) -> Result<VerifyingKey, Reason> {
    let assertion = assertion::read(sdp)?.ok_or("answer has no a=identity")?;
    let jwt = split_jwt(&assertion.token)?;
    if jwt.alg != "ES384" {
        return Err("token alg is not ES384");
    }
    let key = parse_cpk(&jwt.claims["cpk"])?;
    verify_es384(&key, jwt.signing_input.as_bytes(), &jwt.signature)?;
    if jwt.claims["exp"].as_i64().is_some_and(|exp| exp < now_unix) {
        return Err("server identity token expired");
    }
    verify_fingerprints(sdp, &key, &assertion.fingerprints)?;
    Ok(key)
}

/// BDS's answer tokens expire a minute after they are issued.
const TOKEN_LIFETIME: i64 = 60;

pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The identity BDS answers with: domain `self`, and a token self-signed by `key` carrying its
/// SPKI in the `x5u` header and its JWK as `cpk` (layout from a 26.5x capture).
pub(crate) fn server_identity(key: &SigningKey, now_unix: i64) -> Identity {
    let point = key.verifying_key().to_encoded_point(false);
    let coord = |c: Option<&_>| URL_SAFE_NO_PAD.encode(c.expect("uncompressed point"));
    let spki = key.verifying_key().to_public_key_der().expect("P-384 key encodes as SPKI");
    let header = json!({ "alg": "ES384", "x5u": STANDARD.encode(spki.as_bytes()) });
    let claims = json!({
        "cpk": { "kty": "EC", "crv": "P-384", "x": coord(point.x()), "y": coord(point.y()) },
        "exp": now_unix + TOKEN_LIFETIME,
        "iat": now_unix,
    });
    let input = format!("{}.{}", URL_SAFE_NO_PAD.encode(header.to_string()), URL_SAFE_NO_PAD.encode(claims.to_string()));
    let sig: Signature = key.sign(input.as_bytes());
    let token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()));
    Identity { key: key.clone(), token, domain: "self".into() }
}

impl Identity {
    /// A token `key` signs itself, as a client without an account offers it. BDS refuses these;
    /// a host without online auth may take them.
    pub fn self_signed(key: SigningKey) -> Self {
        server_identity(&key, unix_now())
    }
}

/// Adds a BDS-style assertion to a raw answer, for hosts that keep str0m's own SDP layout.
pub fn sign_answer(sdp: &str, key: &SigningKey, now_unix: i64) -> String {
    insert_session_line(sdp, &identity_line(sdp, &server_identity(key, now_unix)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(key: &SigningKey, now_unix: i64) -> String {
        let sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=fingerprint:sha-256 AB:CD\r\n";
        sign_answer(sdp, key, now_unix)
    }

    #[test]
    fn accepts_valid_and_rejects_tampered_or_expired() {
        let key = SigningKey::from_slice(&[3; 48]).unwrap();
        let sdp = answer(&key, 1_000);
        assert_eq!(verify_answer(&sdp, 1_030).unwrap(), *key.verifying_key());
        assert!(verify_answer(&sdp.replace("AB:CD", "AB:CE"), 1_030).is_err());
        assert!(verify_answer(&sdp, 1_000 + TOKEN_LIFETIME + 1).is_err());
        assert!(verify_answer("v=0\r\n", 0).is_err());
    }

    #[test]
    fn token_header_carries_the_spki() {
        let key = SigningKey::from_slice(&[3; 48]).unwrap();
        let identity = server_identity(&key, 1_000);
        let header = assertion::b64url(identity.token.split('.').next().unwrap()).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header).unwrap();
        let spki = STANDARD.decode(header["x5u"].as_str().unwrap()).unwrap();
        assert_eq!(<VerifyingKey as p384::pkcs8::DecodePublicKey>::from_public_key_der(&spki).unwrap(), *key.verifying_key());
    }
}
