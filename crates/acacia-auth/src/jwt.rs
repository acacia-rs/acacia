//! Compact ES384 JWTs as Bedrock uses them: header `{"alg":"ES384","x5u":<SPKI DER b64>}`,
//! signature is raw `r || s` (96 bytes), not DER. RS256 (the real multiplayer token) is verified by
//! `login::verify::SigningKeys`.

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use p384::ecdsa::signature::{Signer, Verifier};
use p384::ecdsa::{Signature, SigningKey, VerifyingKey};
use p384::pkcs8::{DecodePublicKey, EncodePublicKey};
use serde::Serialize;
use serde_json::Value;

use crate::{Error, Result};

/// X.509 SubjectPublicKeyInfo DER of the key's public half, standard base64 (the `x5u` format).
pub fn public_key_der_b64(key: &SigningKey) -> String {
    let der = key
        .verifying_key()
        .to_public_key_der()
        .expect("P-384 SPKI encoding cannot fail");
    STANDARD.encode(der.as_bytes())
}

/// Parses a standard-base64 SPKI DER P-384 public key (an `x5u` / `identityPublicKey` value).
pub fn parse_public_key_der_b64(b64: &str) -> Result<p384::PublicKey> {
    let der = STANDARD
        .decode(b64)
        .map_err(|e| Error::Key(format!("base64: {e}")))?;
    p384::PublicKey::from_public_key_der(&der).map_err(|e| Error::Key(e.to_string()))
}

#[derive(Serialize)]
struct Header<'a> {
    alg: &'static str,
    x5u: &'a str,
}

/// Signs `claims` with ES384; the header's `x5u` is the key's own public key.
pub fn sign(key: &SigningKey, claims: &impl Serialize) -> Result<String> {
    let x5u = public_key_der_b64(key);
    let header = serde_json::to_vec(&Header { alg: "ES384", x5u: &x5u })?;
    let payload = serde_json::to_vec(claims)?;
    let mut out = URL_SAFE_NO_PAD.encode(header);
    out.push('.');
    out.push_str(&URL_SAFE_NO_PAD.encode(payload));
    let sig: Signature = key.sign(out.as_bytes());
    out.push('.');
    out.push_str(&URL_SAFE_NO_PAD.encode(sig.to_bytes()));
    Ok(out)
}

/// A split JWT whose header and claims were decoded but not verified.
pub struct Unverified<'a> {
    pub header: Value,
    pub claims: Value,
    signing_input: &'a str,
    signature: Vec<u8>,
}

impl Unverified<'_> {
    /// The `x5u` header value, if present.
    pub fn x5u(&self) -> Option<&str> {
        self.header.get("x5u").and_then(Value::as_str)
    }

    pub fn alg(&self) -> Option<&str> {
        self.header.get("alg").and_then(Value::as_str)
    }

    pub fn kid(&self) -> Option<&str> {
        self.header.get("kid").and_then(Value::as_str)
    }

    /// The bytes the signature covers: `<header b64>.<claims b64>`.
    pub fn signing_input(&self) -> &[u8] {
        self.signing_input.as_bytes()
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    pub fn verify(&self, key: &p384::PublicKey) -> Result<()> {
        if self.alg() != Some("ES384") {
            return Err(Error::Jwt("alg is not ES384".into()));
        }
        let sig = Signature::from_slice(&self.signature)
            .map_err(|_| Error::Jwt("signature is not 96-byte r||s".into()))?;
        VerifyingKey::from(key)
            .verify(self.signing_input.as_bytes(), &sig)
            .map_err(|_| Error::Jwt("signature mismatch".into()))
    }
}

/// Splits and decodes a compact JWT without verifying it.
pub fn decode(jwt: &str) -> Result<Unverified<'_>> {
    let mut parts = jwt.split('.');
    let (Some(h), Some(c), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Error::Jwt("expected three dot-separated parts".into()));
    };
    let part = |p: &str| {
        URL_SAFE_NO_PAD
            .decode(p.trim_end_matches('='))
            .map_err(|e| Error::Jwt(format!("base64url: {e}")))
    };
    Ok(Unverified {
        header: serde_json::from_slice(&part(h)?)?,
        claims: serde_json::from_slice(&part(c)?)?,
        signing_input: &jwt[..h.len() + 1 + c.len()],
        signature: part(s)?,
    })
}

/// Decodes a JWT and verifies it against the key in its own `x5u` header.
pub fn decode_self_signed(jwt: &str) -> Result<(p384::PublicKey, Value)> {
    let token = decode(jwt)?;
    let key = parse_public_key_der_b64(
        token.x5u().ok_or_else(|| Error::Jwt("missing x5u header".into()))?,
    )?;
    token.verify(&key)?;
    Ok((key, token.claims))
}

pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
