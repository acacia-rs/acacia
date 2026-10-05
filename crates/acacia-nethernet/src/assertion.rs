//! Reading an `a=identity` line and the tokens in it, for both directions
//! (docs/research/nethernet-wire.md §2).

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p384::ecdsa::signature::Verifier;
use p384::ecdsa::{Signature, VerifyingKey};
use p384::pkcs8::DecodePublicKey;
use serde_json::Value;

use crate::identity::{fingerprint_payload, IDENTITY};

/// Why an assertion was refused; the caller says whose it was.
pub(crate) type Reason = &'static str;

pub(crate) struct Assertion {
    pub domain: String,
    pub token: String,
    /// Detached JWS over the SDP's fingerprints.
    pub fingerprints: String,
}

/// The SDP's assertion, or `None` if it has no `a=identity` line.
pub(crate) fn read(sdp: &str) -> Result<Option<Assertion>, Reason> {
    let Some(line) = sdp.lines().find_map(|l| l.strip_prefix(IDENTITY)) else { return Ok(None) };
    let envelope = STANDARD.decode(line.trim()).map_err(|_| "identity is not base64")?;
    let envelope: Value = serde_json::from_slice(&envelope).map_err(|_| "identity is not JSON")?;
    let assertion: Value =
        envelope["assertion"].as_str().and_then(|s| serde_json::from_str(s).ok()).ok_or("assertion is not a JSON string")?;
    Ok(Some(Assertion {
        domain: envelope["idp"]["domain"].as_str().unwrap_or_default().to_owned(),
        token: assertion["token"].as_str().ok_or("assertion has no token")?.to_owned(),
        fingerprints: assertion["fingerprints"].as_str().ok_or("assertion has no fingerprints")?.to_owned(),
    }))
}

pub(crate) struct Jwt<'a> {
    pub alg: String,
    pub claims: Value,
    /// `header.claims`, what the signature covers.
    pub signing_input: &'a str,
    pub signature: Vec<u8>,
}

pub(crate) fn split_jwt(token: &str) -> Result<Jwt<'_>, Reason> {
    let (signing_input, signature) = token.rsplit_once('.').ok_or("token is not a JWT")?;
    let (header, claims) = signing_input.split_once('.').ok_or("token is not a JWT")?;
    let header: Value = serde_json::from_slice(&b64url(header)?).map_err(|_| "token header is not JSON")?;
    Ok(Jwt {
        alg: header["alg"].as_str().unwrap_or_default().to_owned(),
        claims: serde_json::from_slice(&b64url(claims)?).map_err(|_| "token claims are not JSON")?,
        signing_input,
        signature: b64url(signature)?,
    })
}

/// `cpk` as a P-384 JWK object, or (older form) base64 SPKI DER.
pub(crate) fn parse_cpk(cpk: &Value) -> Result<VerifyingKey, Reason> {
    if let Some(der) = cpk.as_str() {
        let der = STANDARD.decode(der).map_err(|_| "cpk is not base64")?;
        return VerifyingKey::from_public_key_der(&der).map_err(|_| "cpk is not a P-384 key");
    }
    if cpk["kty"] != "EC" || cpk["crv"] != "P-384" {
        return Err("cpk is not a P-384 JWK");
    }
    let coord = |c: &str| cpk[c].as_str().ok_or("cpk lacks a coordinate").and_then(b64url);
    let mut sec1 = vec![4u8];
    sec1.extend(coord("x")?);
    sec1.extend(coord("y")?);
    VerifyingKey::from_sec1_bytes(&sec1).map_err(|_| "cpk is not a P-384 point")
}

pub(crate) fn verify_es384(key: &VerifyingKey, input: &[u8], signature: &[u8]) -> Result<(), Reason> {
    let sig = Signature::from_slice(signature).map_err(|_| "signature is not 96-byte r||s")?;
    key.verify(input, &sig).map_err(|_| "signature mismatch")
}

/// Checks that `key` made the detached JWS over the SDP's fingerprints: it owns the DTLS certificate.
pub(crate) fn verify_fingerprints(sdp: &str, key: &VerifyingKey, detached: &str) -> Result<(), Reason> {
    let (header, sig) = detached.split_once("..").ok_or("fingerprints is not a detached JWS")?;
    let payload = URL_SAFE_NO_PAD.encode(fingerprint_payload(sdp.lines()));
    verify_es384(key, format!("{header}.{payload}").as_bytes(), &b64url(sig)?)
}

pub(crate) fn b64url(s: &str) -> Result<Vec<u8>, Reason> {
    URL_SAFE_NO_PAD.decode(s.trim_end_matches('=')).map_err(|_| "bad base64url")
}
