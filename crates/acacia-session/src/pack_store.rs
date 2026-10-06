//! Resource packs an account has downloaded before. A vanilla client answers HaveAllPacks for packs
//! in its cache and downloads the rest (session/packs.rs); bots do the same per account. Only ids
//! are kept: bots never use a pack's contents.

use std::collections::HashSet;
use std::sync::Mutex;

/// Packs by the id servers use in ResourcePackClientResponse: `<uuid>_<version>`.
pub trait PackStore: Send + Sync {
    fn has(&self, id: &str) -> bool;
    /// A pack downloaded in full and checked against the server's hash.
    fn insert(&self, id: &str);
}

/// Answers like a returning player with every pack cached, so nothing is downloaded.
pub struct EveryPack;

impl PackStore for EveryPack {
    fn has(&self, _: &str) -> bool {
        true
    }

    fn insert(&self, _: &str) {}
}

/// A store that lives as long as the connection: every join downloads, like a fresh install.
#[derive(Default)]
pub struct MemoryPackStore {
    ids: Mutex<HashSet<String>>,
}

impl PackStore for MemoryPackStore {
    fn has(&self, id: &str) -> bool {
        self.ids.lock().expect("pack store lock").contains(id)
    }

    fn insert(&self, id: &str) {
        self.ids.lock().expect("pack store lock").insert(id.to_owned());
    }
}
