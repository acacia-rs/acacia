use std::collections::BTreeMap;

use p256::ecdsa::SigningKey as XboxKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

use super::live::MsaToken;
use super::minecraft::{PlayFabSession, ServiceToken};
use super::xbox::{SisuTokens, XboxToken};
use crate::LoginCredentials;

/// Everything reusable for one account. Persisting it (see [`super::TokenCache`]) keeps the Xbox
/// device key and device token stable, which avoids device-token rate limits (gophertunnel
/// `XBLTokenCache` exists for the same reason).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedTokens {
    pub msa: Option<MsaToken>,
    /// P-256 proof key for Xbox request signing, hex-encoded scalar.
    pub xbox_device_key: String,
    /// Device `id` sent to Minecraft session/start.
    pub device_id: String,
    pub device_token: Option<XboxToken>,
    pub sisu: Option<SisuTokens>,
    /// XSTS tokens keyed by relying party.
    #[serde(default)]
    pub xsts: BTreeMap<String, XboxToken>,
    pub playfab: Option<PlayFabSession>,
    pub service_token: Option<ServiceToken>,
    /// Last credentials, reusable while unexpired for the same client key.
    pub credentials: Option<CachedCredentials>,
    /// P-384 client key (hex scalar) that `credentials` is bound to. Reusing it across reconnects
    /// avoids re-requesting the Mojang chain, which is rate limited (HTTP 429).
    #[serde(default)]
    pub client_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedCredentials {
    /// SPKI base64 of the P-384 client key the chain and multiplayer token are bound to.
    pub client_public_key: String,
    pub credentials: LoginCredentials,
}

impl CachedTokens {
    pub fn new(msa: Option<MsaToken>) -> Self {
        Self {
            msa,
            xbox_device_key: hex::encode(XboxKey::random(&mut OsRng).to_bytes()),
            device_id: uuid::Uuid::new_v4().to_string(),
            device_token: None,
            sisu: None,
            xsts: BTreeMap::new(),
            playfab: None,
            service_token: None,
            credentials: None,
            client_key: None,
        }
    }

    /// The persisted client key, created (dropping bound credentials) if missing or unreadable.
    pub(crate) fn client_key(&mut self) -> p384::ecdsa::SigningKey {
        let parsed = self
            .client_key
            .as_deref()
            .and_then(|h| hex::decode(h).ok())
            .and_then(|b| p384::ecdsa::SigningKey::from_slice(&b).ok());
        parsed.unwrap_or_else(|| {
            let key = p384::ecdsa::SigningKey::random(&mut OsRng);
            self.client_key = Some(hex::encode(key.to_bytes()));
            self.credentials = None;
            key
        })
    }

    /// The persisted proof key; regenerated (dropping the device token) if the stored one is bad.
    pub(crate) fn xbox_key(&mut self) -> XboxKey {
        let parsed = hex::decode(&self.xbox_device_key)
            .ok()
            .and_then(|b| XboxKey::from_slice(&b).ok());
        parsed.unwrap_or_else(|| {
            let key = XboxKey::random(&mut OsRng);
            self.xbox_device_key = hex::encode(key.to_bytes());
            self.device_token = None;
            self.sisu = None;
            self.xsts.clear();
            key
        })
    }

    /// Drops everything derived from the MSA token (after a new login).
    pub(crate) fn reset_user_tokens(&mut self) {
        self.sisu = None;
        self.xsts.clear();
        self.playfab = None;
        self.service_token = None;
        self.credentials = None;
    }
}
