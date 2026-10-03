use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

use p256::ecdsa::SigningKey as XboxKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

use super::live::MsaToken;
use super::minecraft::{PlayFabSession, ServiceToken};
use super::xbox::{SisuTokens, XboxToken};
use crate::LoginCredentials;

/// Everything reusable for one account. Persisting it (see [`TokenCache`]) keeps the Xbox device
/// key and device token stable, which avoids device-token rate limits (gophertunnel
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

pub trait TokenCache: Send + Sync {
    fn load(&self, account: &str) -> Option<CachedTokens>;
    fn store(&self, account: &str, tokens: &CachedTokens);
}

#[derive(Default)]
pub struct MemoryTokenCache {
    entries: Mutex<HashMap<String, CachedTokens>>,
}

impl MemoryTokenCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenCache for MemoryTokenCache {
    fn load(&self, account: &str) -> Option<CachedTokens> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).get(account).cloned()
    }

    fn store(&self, account: &str, tokens: &CachedTokens) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(account.to_owned(), tokens.clone());
    }
}

/// One `<account>.json` per account in a directory. Writes go to a temp file then rename, so a
/// crash never leaves a truncated cache. The files hold refresh tokens: keep the directory private.
pub struct FileTokenCache {
    dir: PathBuf,
}

impl FileTokenCache {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn path_for(&self, account: &str) -> PathBuf {
        let safe: String = account
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || "-_.@".contains(c) { c } else { '_' })
            .collect();
        self.dir.join(format!("{safe}.json"))
    }
}

impl TokenCache for FileTokenCache {
    fn load(&self, account: &str) -> Option<CachedTokens> {
        let bytes = std::fs::read(self.path_for(account)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn store(&self, account: &str, tokens: &CachedTokens) {
        let path = self.path_for(account);
        let tmp = path.with_extension("json.tmp");
        let Ok(bytes) = serde_json::to_vec_pretty(tokens) else { return };
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CachedTokens {
        let mut t = CachedTokens::new(Some(MsaToken {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: 42,
            user_id: None,
        }));
        t.xsts.insert("rp".into(), XboxToken {
            token: "x".into(),
            not_after: 1,
            user_hash: Some("uhs".into()),
            xuid: None,
            gamertag: None,
        });
        t
    }

    #[test]
    fn memory_round_trip() {
        let cache = MemoryTokenCache::new();
        assert!(cache.load("a").is_none());
        let tokens = sample();
        cache.store("a", &tokens);
        assert_eq!(cache.load("a"), Some(tokens));
    }

    #[test]
    fn file_round_trip_keeps_device_key() {
        let dir = std::env::temp_dir().join(format!("acacia-auth-test-{}", uuid::Uuid::new_v4()));
        let cache = FileTokenCache::new(&dir).unwrap();
        let mut tokens = sample();
        let key = tokens.xbox_key();
        cache.store("user@example.com", &tokens);
        let mut loaded = cache.load("user@example.com").unwrap();
        assert_eq!(loaded, tokens);
        assert_eq!(loaded.xbox_key().to_bytes(), key.to_bytes());
        assert!(cache.path_for("a/b").ends_with("a_b.json"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
