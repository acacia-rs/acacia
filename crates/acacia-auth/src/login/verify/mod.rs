//! Server-side Login verification, network-free: who is joining, and whether Xbox Live vouches for
//! it. Rules and their gophertunnel sources: docs/auth.md "Verifying logins (servers)".

mod chain;
#[cfg(test)]
mod chain_tests;
mod envelope;
mod error;
#[cfg(test)]
mod fixtures;
mod jws;
mod keys;
#[cfg(test)]
mod malformed_tests;
#[cfg(test)]
mod tests;
mod token;

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

pub use chain::MOJANG_ROOT_KEY;
pub use error::{Part, VerifyError};
pub use keys::SigningKeys;

use super::{ClientData, MULTIPLAYER_AUDIENCE};
use crate::jwt;

/// `iss` of multiplayer tokens; the trailing slash is part of it.
pub const MULTIPLAYER_ISSUER: &str = "https://authorization.franchise.minecraft-services.net/";

const DEFAULT_LEEWAY_SECS: i64 = 60;

/// Who a Login names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// Empty unless the login is authenticated: an XUID nobody vouches for is dropped.
    pub xuid: String,
    pub display_name: String,
    /// Authenticated: derived from the XUID ([`crate::login::xuid_identity`]) or issued by Mojang
    /// in the chain. Otherwise whatever the client claims, or derived from its name.
    pub uuid: Uuid,
    /// Xbox title id (`extraData.titleId`), only in a Mojang-signed chain.
    pub title_id: Option<String>,
    /// PlayFab master player id (`mid`), only in a service-issued token.
    pub playfab_id: Option<String>,
}

/// A Login whose signatures all check out.
#[derive(Debug, Clone)]
pub struct VerifiedLogin {
    pub identity: Identity,
    /// True only when the authorization service or Mojang signed the identity and it has an XUID.
    /// False for self-signed (offline) logins: `identity` is then the client's own claim.
    pub authenticated: bool,
    /// The key that signed the client data: the ECDH peer for the encryption handshake.
    pub client_key: p384::PublicKey,
    pub client_data: ClientData,
    /// The client-data claims as sent, including the ones [`ClientData`] does not model.
    pub client_claims: Value,
}

/// What both credential forms resolve to.
struct Subject {
    identity: Identity,
    key: p384::PublicKey,
    authenticated: bool,
}

/// Verifies Login connection requests against a snapshot of the issuer's keys.
#[derive(Debug, Clone)]
pub struct Verifier {
    keys: SigningKeys,
    issuer: String,
    audience: String,
    chain_root: p384::PublicKey,
    leeway: i64,
}

impl Verifier {
    /// For the production issuer, audience and Mojang root key, with 60 s of clock leeway.
    pub fn new(keys: SigningKeys) -> Self {
        Self {
            keys,
            issuer: MULTIPLAYER_ISSUER.into(),
            audience: MULTIPLAYER_AUDIENCE.into(),
            chain_root: jwt::parse_public_key_der_b64(MOJANG_ROOT_KEY).expect("MOJANG_ROOT_KEY is a P-384 key"),
            leeway: DEFAULT_LEEWAY_SECS,
        }
    }

    /// Replaces the key snapshot, e.g. after a JWKS refresh.
    pub fn set_keys(&mut self, keys: SigningKeys) {
        self.keys = keys;
    }

    pub fn keys(&self) -> &SigningKeys {
        &self.keys
    }

    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = issuer.into();
        self
    }

    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = audience.into();
        self
    }

    /// Trusts another key as the legacy chain's root (test servers).
    pub fn with_chain_root(mut self, root: p384::PublicKey) -> Self {
        self.chain_root = root;
        self
    }

    /// Seconds of clock skew tolerated on `exp`/`nbf`.
    pub fn with_leeway(mut self, seconds: i64) -> Self {
        self.leeway = seconds.max(0);
        self
    }

    /// Verifies the connection request of a Login packet (the bytes after the protocol version)
    /// at `now` (unix seconds). A token takes precedence over a chain, as in gophertunnel.
    pub fn verify(&self, request: &[u8], now: i64) -> Result<VerifiedLogin, VerifyError> {
        let request = envelope::parse(request)?;
        let subject = match request.token.as_deref() {
            Some(token) => {
                let mut subject = token::verify(self, token, now)?;
                subject.identity.title_id = self.chain_title_id(&subject, &request.chain, now);
                subject
            }
            None if request.chain.is_empty() => return Err(VerifyError::NoCredentials),
            None => chain::verify(&self.chain_root, &request.chain, now, self.leeway)?,
        };

        let part = Part::ClientData;
        let client = jws::decode(part, request.client_jwt)?;
        jws::verify_es384(part, &client, &subject.key)?;
        let client_data =
            ClientData::deserialize(&client.claims).map_err(|e| VerifyError::Malformed { part, reason: e.to_string() })?;
        Ok(VerifiedLogin {
            identity: subject.identity,
            authenticated: subject.authenticated,
            client_key: subject.key,
            client_data,
            client_claims: client.claims,
        })
    }

    /// The token has no Xbox title id; a valid Mojang chain for the same XUID supplies it.
    fn chain_title_id(&self, subject: &Subject, chain: &[String], now: i64) -> Option<String> {
        if !subject.authenticated || chain.is_empty() {
            return None;
        }
        let legacy = chain::verify(&self.chain_root, chain, now, self.leeway).ok()?;
        (legacy.authenticated && legacy.identity.xuid == subject.identity.xuid).then_some(legacy.identity.title_id)?
    }
}
