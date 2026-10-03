//! A per-account record of downloaded resource packs on disk, so a returning bot answers HaveAllPacks
//! like a vanilla client with the packs cached, and a new account downloads them. File: one
//! `<uuid>_<version>` per line, appended.

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use acacia_session::pack_store::{MemoryPackStore, PackStore};
use sha2::{Digest, Sha256};

/// `dir` holds one file per account; without a dir every join downloads, like a fresh install.
pub(crate) fn open(dir: Option<&Path>, account: &str) -> Arc<dyn PackStore> {
    let Some(dir) = dir else { return Arc::new(MemoryPackStore::default()) };
    let path = DiskPackStore::path_for(dir, account);
    match DiskPackStore::open(&path) {
        Ok(store) => Arc::new(store),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "pack cache unavailable; using memory");
            Arc::new(MemoryPackStore::default())
        }
    }
}

pub struct DiskPackStore {
    path: PathBuf,
    ids: Mutex<HashSet<String>>,
}

impl DiskPackStore {
    /// One file per account, so accounts never share what they claim to hold.
    pub fn path_for(dir: &Path, account: &str) -> PathBuf {
        let digest = Sha256::digest(format!("bedrock-packs:{account}"));
        let name: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        dir.join(format!("{name}.packs"))
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let ids = match std::fs::read_to_string(path) {
            Ok(text) => text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => HashSet::new(),
            Err(e) => return Err(e),
        };
        Ok(Self { path: path.to_owned(), ids: Mutex::new(ids) })
    }
}

impl PackStore for DiskPackStore {
    fn has(&self, id: &str) -> bool {
        self.ids.lock().expect("pack cache lock").contains(id)
    }

    fn insert(&self, id: &str) {
        if !self.ids.lock().expect("pack cache lock").insert(id.to_owned()) {
            return;
        }
        let written = OpenOptions::new().create(true).append(true).open(&self.path).and_then(|mut f| writeln!(f, "{id}"));
        if let Err(e) = written {
            tracing::warn!(error = %e, "pack cache write failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_packs_per_account_across_opens() {
        let dir = std::env::temp_dir().join(format!("bedrock-packs-{}", std::process::id()));
        let id = "8fbd6e02-fb25-4e1d-adc2-776df9dfb30b_1.0.0";
        open(Some(&dir), "xbox:1").insert(id);
        assert!(open(Some(&dir), "xbox:1").has(id));
        assert!(!open(Some(&dir), "xbox:2").has(id), "accounts don't share packs");
        assert!(!open(None, "xbox:1").has(id), "no dir: a fresh install every join");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
