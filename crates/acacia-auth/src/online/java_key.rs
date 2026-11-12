//! Java Edition profile keys (`/player/certificates`): the RSA key pair secure chat signs with.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::account::Account;
use super::java::JavaProfile;
use crate::{Error, Result};

const CERTIFICATES: &str = "https://api.minecraftservices.com/player/certificates";

/// A profile key pair, as vanilla's `ProfileKeyPair` holds it. `Debug` leaves the keys out.
#[derive(Clone, PartialEq, Eq)]
pub struct JavaProfileKey {
    /// PKCS#8 DER.
    pub private_key: Vec<u8>,
    /// X.509 SubjectPublicKeyInfo DER, as `ProfilePublicKey.Data` sends it.
    pub public_key: Vec<u8>,
    /// Mojang's signature over the profile id, expiry and public key (`publicKeySignatureV2`).
    pub signature: Vec<u8>,
    /// Unix milliseconds, truncated like `Instant.toEpochMilli`: the signature covers this exact value.
    pub expires_at: i64,
    /// Unix milliseconds after which vanilla fetches a new pair.
    pub refreshed_after: i64,
}

impl std::fmt::Debug for JavaProfileKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaProfileKey").field("expires_at", &self.expires_at).field("refreshed_after", &self.refreshed_after).finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Certificates {
    key_pair: KeyPair,
    public_key_signature_v2: String,
    expires_at: String,
    refreshed_after: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyPair {
    private_key: String,
    public_key: String,
}

impl Account {
    /// A fresh profile key pair for `player` (a POST, as `ProfileKeyPairManager` makes it).
    pub async fn java_profile_key(&self, player: &JavaProfile) -> Result<JavaProfileKey> {
        let http = self.client().http();
        let c: Certificates = http.json(http.client.post(CERTIFICATES).bearer_auth(&player.access_token), CERTIFICATES).await?;
        Ok(JavaProfileKey {
            private_key: pem_body(&c.key_pair.private_key)?,
            public_key: pem_body(&c.key_pair.public_key)?,
            signature: base64(&c.public_key_signature_v2)?,
            expires_at: millis(&c.expires_at)?,
            refreshed_after: millis(&c.refreshed_after)?,
        })
    }
}

fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::Protocol { endpoint: CERTIFICATES.into(), reason: reason.to_string() }
}

/// The DER inside a PEM block. The labels say `RSA ... KEY` but the bodies are PKCS#8 and SPKI.
fn pem_body(pem: &str) -> Result<Vec<u8>> {
    base64(&pem.lines().filter(|l| !l.starts_with("-----")).collect::<String>())
}

fn base64(text: &str) -> Result<Vec<u8>> {
    let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(clean).map_err(invalid)
}

fn millis(iso: &str) -> Result<i64> {
    let at = OffsetDateTime::parse(iso, &Rfc3339).map_err(|e| invalid(format!("{iso}: {e}")))?;
    Ok(at.unix_timestamp_nanos().div_euclid(1_000_000) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instants_truncate_to_millis_like_java() {
        assert_eq!(millis("2026-11-13T10:00:00.123987654Z").unwrap(), 1_794_564_000_123);
        assert_eq!(millis("2026-11-13T10:00:00Z").unwrap(), 1_794_564_000_000);
    }

    #[test]
    fn pem_bodies_ignore_labels_and_breaks() {
        assert_eq!(pem_body("-----BEGIN RSA PUBLIC KEY-----\nAAEC\nAw==\n-----END RSA PUBLIC KEY-----\n").unwrap(), [0, 1, 2, 3]);
    }
}
