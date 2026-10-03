//! Verifies the `a=identity` every answer must carry: a JWT self-signed by its `cpk` claim and a
//! detached JWS by that key over the answer's fingerprints. Trust in the key itself is the
//! caller's (pin it); this proves the server holds it and owns the DTLS certificate.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p384::ecdsa::signature::Verifier;
use p384::ecdsa::{Signature, VerifyingKey};
use p384::pkcs8::DecodePublicKey;
use serde_json::Value;

use crate::identity::{fingerprint_payload, IDENTITY};
use crate::Error;

fn bad(reason: &str) -> Error {
    Error::ServerIdentity(reason.to_owned())
}

/// Checks the answer's assertion and returns the server's key (`cpk`).
pub(crate) fn verify_answer(sdp: &str, now_unix: i64) -> Result<VerifyingKey, Error> {
    let line = sdp.lines().find_map(|l| l.strip_prefix(IDENTITY)).ok_or_else(|| bad("answer has no a=identity"))?;
    let envelope: Value = serde_json::from_slice(&STANDARD.decode(line.trim()).map_err(|_| bad("identity is not base64"))?)
        .map_err(|_| bad("identity is not JSON"))?;
    let assertion: Value = envelope["assertion"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or_else(|| bad("assertion is not a JSON string"))?;
    let token = assertion["token"].as_str().ok_or_else(|| bad("assertion has no token"))?;
    let fingerprints = assertion["fingerprints"].as_str().ok_or_else(|| bad("assertion has no fingerprints"))?;

    let (signing_input, claims, signature) = split_jwt(token)?;
    let key = parse_cpk(&claims["cpk"])?;
    verify_es384(&key, signing_input.as_bytes(), &signature)?;
    if claims["exp"].as_i64().is_some_and(|exp| exp < now_unix) {
        return Err(bad("server identity token expired"));
    }

    let (header, sig) = fingerprints.split_once("..").ok_or_else(|| bad("fingerprints is not a detached JWS"))?;
    let payload = URL_SAFE_NO_PAD.encode(fingerprint_payload(sdp.lines()));
    verify_es384(&key, format!("{header}.{payload}").as_bytes(), &b64url(sig)?)?;
    Ok(key)
}

fn split_jwt(token: &str) -> Result<(&str, Value, Vec<u8>), Error> {
    let (input, sig) = token.rsplit_once('.').ok_or_else(|| bad("token is not a JWT"))?;
    let (header, claims) = input.split_once('.').ok_or_else(|| bad("token is not a JWT"))?;
    let header: Value = serde_json::from_slice(&b64url(header)?).map_err(|_| bad("token header is not JSON"))?;
    if header["alg"] != "ES384" {
        return Err(bad("token alg is not ES384"));
    }
    let claims = serde_json::from_slice(&b64url(claims)?).map_err(|_| bad("token claims are not JSON"))?;
    Ok((input, claims, b64url(sig)?))
}

/// `cpk` as a P-384 JWK object, or (older form) base64 SPKI DER.
fn parse_cpk(cpk: &Value) -> Result<VerifyingKey, Error> {
    if let Some(der) = cpk.as_str() {
        let der = STANDARD.decode(der).map_err(|_| bad("cpk is not base64"))?;
        return VerifyingKey::from_public_key_der(&der).map_err(|_| bad("cpk is not a P-384 key"));
    }
    if cpk["kty"] != "EC" || cpk["crv"] != "P-384" {
        return Err(bad("cpk is not a P-384 JWK"));
    }
    let coord = |c: &str| cpk[c].as_str().ok_or_else(|| bad("cpk lacks a coordinate")).and_then(b64url);
    let mut sec1 = vec![4u8];
    sec1.extend(coord("x")?);
    sec1.extend(coord("y")?);
    VerifyingKey::from_sec1_bytes(&sec1).map_err(|_| bad("cpk is not a P-384 point"))
}

fn verify_es384(key: &VerifyingKey, input: &[u8], signature: &[u8]) -> Result<(), Error> {
    let sig = Signature::from_slice(signature).map_err(|_| bad("signature is not 96-byte r||s"))?;
    key.verify(input, &sig).map_err(|_| bad("signature mismatch"))
}

fn b64url(s: &str) -> Result<Vec<u8>, Error> {
    URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).map_err(|_| bad("bad base64url"))
}

/// Adds a BDS-style assertion to `sdp`: a self-signed JWT with a JWK `cpk`, signed over the fingerprints.
#[cfg(any(test, feature = "test-support"))]
pub fn sign_as_server(sdp: &str, key: &p384::ecdsa::SigningKey, exp: i64) -> String {
    use crate::identity::{identity_line, insert_session_line, Identity};
    use p384::ecdsa::signature::Signer;
    let point = key.verifying_key().to_encoded_point(false);
    let claims = serde_json::json!({
        "cpk": {"kty": "EC", "crv": "P-384", "x": URL_SAFE_NO_PAD.encode(point.x().unwrap()), "y": URL_SAFE_NO_PAD.encode(point.y().unwrap())},
        "exp": exp,
    });
    let input = format!("{}.{}", URL_SAFE_NO_PAD.encode(r#"{"alg":"ES384"}"#), URL_SAFE_NO_PAD.encode(claims.to_string()));
    let sig: Signature = key.sign(input.as_bytes());
    let token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()));
    insert_session_line(sdp, &identity_line(sdp, &Identity { key: key.clone(), token, domain: "self".into() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p384::ecdsa::SigningKey;

    fn answer(key: &SigningKey, exp: i64) -> String {
        let sdp = "v=0\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\na=fingerprint:sha-256 AB:CD\r\n";
        sign_as_server(sdp, key, exp)
    }

    #[test]
    fn accepts_valid_and_rejects_tampered_or_expired() {
        let key = SigningKey::from_slice(&[3; 48]).unwrap();
        let sdp = answer(&key, 2_000_000_000);
        assert_eq!(verify_answer(&sdp, 1_000).unwrap(), *key.verifying_key());
        assert!(verify_answer(&sdp.replace("AB:CD", "AB:CE"), 1_000).is_err());
        assert!(verify_answer(&answer(&key, 10), 1_000).is_err());
        assert!(verify_answer("v=0\r\n", 0).is_err());
    }
}
