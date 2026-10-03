use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::tokens::CachedTokens;

/// Persists accounts' tokens. Writes are compare-and-swap on a per-account version, so two
/// writers (nodes sharing one cache) cannot overwrite each other's rotated refresh tokens.
#[async_trait::async_trait]
pub trait TokenCache: Send + Sync {
    async fn load(&self, account: &str) -> Result<Option<Versioned>, CacheError>;
    /// Writes only if the stored version is still `expected` (`None`: only if nothing is stored).
    async fn store(&self, account: &str, tokens: &CachedTokens, expected: Option<u64>) -> Result<Stored, CacheError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versioned {
    pub tokens: CachedTokens,
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    Written { version: u64 },
    /// Someone else wrote since `expected`; nothing was stored.
    Conflict,
}

#[derive(Debug, thiserror::Error)]
#[error("token cache: {0}")]
pub struct CacheError(Box<dyn std::error::Error + Send + Sync>);

impl CacheError {
    pub fn new(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(error.into())
    }
}

fn swap(current: Option<u64>, expected: Option<u64>) -> Option<u64> {
    (current == expected).then(|| current.map_or(1, |v| v + 1))
}

#[derive(Default)]
pub struct MemoryTokenCache {
    entries: Mutex<HashMap<String, Versioned>>,
}

impl MemoryTokenCache {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl TokenCache for MemoryTokenCache {
    async fn load(&self, account: &str) -> Result<Option<Versioned>, CacheError> {
        Ok(self.entries.lock().unwrap_or_else(|e| e.into_inner()).get(account).cloned())
    }

    async fn store(&self, account: &str, tokens: &CachedTokens, expected: Option<u64>) -> Result<Stored, CacheError> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let Some(version) = swap(entries.get(account).map(|v| v.version), expected) else { return Ok(Stored::Conflict) };
        entries.insert(account.to_owned(), Versioned { tokens: tokens.clone(), version });
        Ok(Stored::Written { version })
    }
}

/// One `<account>.json` per account in a directory. Writes go to a temp file then rename, so a
/// crash never leaves a truncated cache. The files hold refresh tokens: keep the directory private.
/// Versions are checked within this process only; don't point two processes at one directory.
pub struct FileTokenCache {
    dir: PathBuf,
    write: Mutex<()>,
}

#[derive(Serialize, Deserialize)]
struct FileEntry {
    /// Absent in files written before versioning.
    #[serde(default)]
    version: u64,
    #[serde(flatten)]
    tokens: CachedTokens,
}

impl FileTokenCache {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, write: Mutex::new(()) })
    }

    pub fn path_for(&self, account: &str) -> PathBuf {
        let safe: String = account
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || "-_.@".contains(c) { c } else { '_' })
            .collect();
        self.dir.join(format!("{safe}.json"))
    }

    fn read(&self, account: &str) -> Result<Option<Versioned>, CacheError> {
        let bytes = match std::fs::read(self.path_for(account)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(CacheError::new(e)),
        };
        let entry: FileEntry = serde_json::from_slice(&bytes).map_err(CacheError::new)?;
        Ok(Some(Versioned { tokens: entry.tokens, version: entry.version }))
    }
}

#[async_trait::async_trait]
impl TokenCache for FileTokenCache {
    async fn load(&self, account: &str) -> Result<Option<Versioned>, CacheError> {
        self.read(account)
    }

    async fn store(&self, account: &str, tokens: &CachedTokens, expected: Option<u64>) -> Result<Stored, CacheError> {
        let _write = self.write.lock().unwrap_or_else(|e| e.into_inner());
        let Some(version) = swap(self.read(account)?.map(|v| v.version), expected) else { return Ok(Stored::Conflict) };
        let path = self.path_for(account);
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&FileEntry { version, tokens: tokens.clone() }).map_err(CacheError::new)?;
        std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, &path)).map_err(CacheError::new)?;
        Ok(Stored::Written { version })
    }
}

#[cfg(test)]
mod tests;
