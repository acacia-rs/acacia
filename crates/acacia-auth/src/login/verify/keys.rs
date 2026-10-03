use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::signature::Verifier as _;
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPublicKey};
use serde::Deserialize;
use sha2::Sha256;

use super::{Part, VerifyError};
use crate::jwt::Unverified;

const MIN_MODULUS_BITS: usize = 2048;

/// The issuer's RS256 keys, a snapshot of its JWKS. Fetching and refreshing it is the caller's job
/// (`AuthClient::fetch_signing_keys` under the `online` feature); see docs/auth.md.
#[derive(Debug, Clone, Default)]
pub struct SigningKeys {
    keys: Vec<(Option<String>, VerifyingKey<Sha256>)>,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

/// `x5t` is ignored: the service sends it as hex, which strict JOSE parsers reject.
#[derive(Deserialize)]
struct Jwk {
    kty: String,
    #[serde(rename = "use")]
    usage: Option<String>,
    alg: Option<String>,
    kid: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

impl SigningKeys {
    /// No keys: every RS256 token is refused, self-signed logins still verify.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Parses a JWKS document (`{"keys":[..]}`), keeping its RSA signing keys. Fails if a kept key
    /// is malformed or none is usable.
    pub fn from_jwks(json: &str) -> Result<Self, VerifyError> {
        let jwks: Jwks = serde_json::from_str(json).map_err(|e| VerifyError::Jwks(e.to_string()))?;
        let usable = |k: &Jwk| k.kty == "RSA" && k.usage.as_deref().is_none_or(|u| u == "sig") && k.alg.as_deref().is_none_or(|a| a == "RS256");
        let keys = jwks.keys.into_iter().filter(usable).map(rsa_key).collect::<Result<Vec<_>, _>>()?;
        if keys.is_empty() {
            return Err(VerifyError::Jwks("no RSA signing keys".into()));
        }
        Ok(Self { keys })
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Whether a key with this `kid` is present; an unknown `kid` is the cue to re-fetch the JWKS.
    pub fn contains(&self, kid: &str) -> bool {
        self.keys.iter().any(|(k, _)| k.as_deref() == Some(kid))
    }

    /// Verifies an RS256 token against the key its `kid` names (any key if it names none).
    pub(super) fn verify(&self, token: &Unverified) -> Result<(), VerifyError> {
        let kid = token.kid();
        let mut candidates = self.keys.iter().filter(|(k, _)| kid.is_none() || k.as_deref() == kid).peekable();
        if candidates.peek().is_none() {
            return Err(VerifyError::UnknownSigningKey { kid: kid.map(str::to_owned) });
        }
        let signature = Signature::try_from(token.signature()).map_err(|_| VerifyError::Signature { part: Part::Token })?;
        if candidates.any(|(_, key)| key.verify(token.signing_input(), &signature).is_ok()) {
            return Ok(());
        }
        Err(VerifyError::Signature { part: Part::Token })
    }
}

fn rsa_key(jwk: Jwk) -> Result<(Option<String>, VerifyingKey<Sha256>), VerifyError> {
    let field = |name: &str, value: &Option<String>| {
        let b64 = value.as_deref().ok_or_else(|| VerifyError::Jwks(format!("RSA key without {name}")))?;
        let bytes = URL_SAFE_NO_PAD.decode(b64.trim_end_matches('=')).map_err(|e| VerifyError::Jwks(format!("{name}: {e}")))?;
        Ok::<_, VerifyError>(BigUint::from_bytes_be(&bytes))
    };
    let key = RsaPublicKey::new(field("n", &jwk.n)?, field("e", &jwk.e)?).map_err(|e| VerifyError::Jwks(e.to_string()))?;
    if key.n().bits() < MIN_MODULUS_BITS {
        return Err(VerifyError::Jwks(format!("{}-bit RSA key is too short", key.n().bits())));
    }
    Ok((jwk.kid, VerifyingKey::new(key)))
}
