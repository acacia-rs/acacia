//! A per-account blob cache on disk, so a returning bot reports chunk blobs it saw in earlier sessions
//! as held, like the vanilla client's cache (docs/research/blob-cache.md §3).
//!
//! File: append-only records `id u64le ‖ len u32le ‖ payload` (`len` 0 = hash only); the newest record
//! for an id wins. Opening compacts the file to the size cap, dropping the oldest records.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use acacia_session::blob_store::{BlobStore, MemoryBlobStore};
use bytes::Bytes;
use sha2::{Digest, Sha256};

const HEADER: usize = 12;

/// Cap per account file when payloads are kept (hash-only files stay tiny).
const MAX_BYTES: u64 = 64 << 20;

/// Where a connection's blob cache lives ([`crate::ClientBuilder::blob_cache_dir`] and friends).
pub(crate) enum BlobCache {
    Off,
    /// For this connection only: every join looks like a fresh install.
    Memory,
    /// `<dir>/<per-account file>`, kept across joins like vanilla's cache.
    Dir(PathBuf),
    Store(Arc<dyn BlobStore>),
}

impl BlobCache {
    pub fn open(&self, account: &str, payloads: bool) -> Option<Arc<dyn BlobStore>> {
        let memory = || -> Arc<dyn BlobStore> {
            Arc::new(if payloads { MemoryBlobStore::with_payloads() } else { MemoryBlobStore::hashes_only() })
        };
        match self {
            Self::Off => None,
            Self::Memory => Some(memory()),
            Self::Store(store) => Some(store.clone()),
            Self::Dir(dir) => {
                let path = DiskBlobStore::path_for(dir, account);
                match DiskBlobStore::open(&path, payloads, MAX_BYTES) {
                    Ok(store) => Some(Arc::new(store)),
                    Err(e) => {
                        tracing::warn!(path = %path.display(), error = %e, "blob cache unavailable; using memory");
                        Some(memory())
                    }
                }
            }
        }
    }
}

pub struct DiskBlobStore {
    payloads: bool,
    inner: Mutex<Inner>,
}

struct Inner {
    file: File,
    /// id → (payload offset, payload length).
    index: HashMap<u64, (u64, u32)>,
    end: u64,
}

impl DiskBlobStore {
    /// One file per account: accounts never share a cache, so their reports stay unrelated.
    pub fn path_for(dir: &Path, account: &str) -> PathBuf {
        let digest = Sha256::digest(format!("bedrock-blobs:{account}"));
        dir.join(format!("{}.blobs", hex(&digest[..8])))
    }

    /// Opens or creates `path`; with `payloads` off it records hashes only (idle bots never read terrain).
    pub fn open(path: &Path, payloads: bool, max_bytes: u64) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut data = Vec::new();
        if let Ok(mut f) = File::open(path) {
            f.read_to_end(&mut data)?;
        }
        let mut records = parse(&data);
        if data.len() as u64 > max_bytes {
            records = newest_within(records, max_bytes * 3 / 4);
            let mut compact = Vec::new();
            for &(id, start, len) in &records {
                compact.extend_from_slice(&id.to_le_bytes());
                compact.extend_from_slice(&len.to_le_bytes());
                compact.extend_from_slice(&data[start..start + len as usize]);
            }
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, &compact)?;
            std::fs::rename(&tmp, path)?;
            records = parse(&compact);
            data = compact;
        }
        let file = OpenOptions::new().create(true).read(true).append(true).open(path)?;
        let index = records.into_iter().map(|(id, start, len)| (id, (start as u64, len))).collect();
        Ok(Self { payloads, inner: Mutex::new(Inner { file, index, end: data.len() as u64 }) })
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("blob cache lock")
    }
}

impl BlobStore for DiskBlobStore {
    fn has(&self, id: u64) -> bool {
        self.inner().index.get(&id).is_some_and(|&(_, len)| !self.payloads || len > 0)
    }

    fn get(&self, id: u64) -> Option<Bytes> {
        if !self.payloads {
            return None;
        }
        let mut inner = self.inner();
        let (offset, len) = *inner.index.get(&id).filter(|(_, len)| *len > 0)?;
        let mut buf = vec![0; len as usize];
        let read = inner.file.seek(SeekFrom::Start(offset)).and_then(|_| inner.file.read_exact(&mut buf));
        read.inspect_err(|e| tracing::warn!(error = %e, "blob cache read failed")).ok()?;
        Some(buf.into())
    }

    fn insert(&self, id: u64, blob: &Bytes) {
        if self.has(id) {
            return;
        }
        let payload: &[u8] = if self.payloads { blob } else { &[] };
        let mut record = Vec::with_capacity(HEADER + payload.len());
        record.extend_from_slice(&id.to_le_bytes());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(payload);
        let mut inner = self.inner();
        if let Err(e) = inner.file.write_all(&record) {
            tracing::warn!(error = %e, "blob cache write failed");
            return;
        }
        let offset = inner.end + HEADER as u64;
        inner.end += record.len() as u64;
        inner.index.insert(id, (offset, payload.len() as u32));
    }

    fn keeps_payloads(&self) -> bool {
        self.payloads
    }
}

/// `(id, payload start, payload len)` per complete record, in file order; a torn tail is ignored.
fn parse(data: &[u8]) -> Vec<(u64, usize, u32)> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(header) = data.get(pos..pos + HEADER) {
        let id = u64::from_le_bytes(header[..8].try_into().expect("8 bytes"));
        let len = u32::from_le_bytes(header[8..].try_into().expect("4 bytes"));
        let start = pos + HEADER;
        if data.len() < start + len as usize {
            break;
        }
        out.push((id, start, len));
        pos = start + len as usize;
    }
    out
}

/// The newest record per id, newest first until `budget` bytes, returned in file order.
fn newest_within(records: Vec<(u64, usize, u32)>, budget: u64) -> Vec<(u64, usize, u32)> {
    let mut seen = std::collections::HashSet::new();
    let mut used = 0u64;
    let mut kept: Vec<_> = records
        .into_iter()
        .rev()
        .filter(|&(id, _, _)| seen.insert(id))
        .take_while(|&(_, _, len)| {
            used += HEADER as u64 + u64::from(len);
            used <= budget
        })
        .collect();
    kept.reverse();
    kept
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("bedrock-blobs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn survives_reopen_and_compacts_oldest_first() {
        let path = temp("reopen");
        let blob = |n: u8| Bytes::from(vec![n; 100]);
        {
            let store = DiskBlobStore::open(&path, true, 1 << 20).unwrap();
            for id in 1..=10u64 {
                store.insert(id, &blob(id as u8));
            }
            assert_eq!(store.get(3), Some(blob(3)));
        }
        let store = DiskBlobStore::open(&path, true, 1 << 20).unwrap();
        assert!(store.has(1) && store.has(10));
        assert_eq!(store.get(7), Some(blob(7)));
        drop(store);

        // 10 records of 112 B; a 600 B cap keeps the newest that fit in 450 B.
        let store = DiskBlobStore::open(&path, true, 600).unwrap();
        assert!(!store.has(6) && store.has(7) && store.has(10));
        assert_eq!(store.get(10), Some(blob(10)));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn hash_only_records_do_not_count_for_payload_stores() {
        let path = temp("lean");
        DiskBlobStore::open(&path, false, 1 << 20).unwrap().insert(5, &Bytes::from_static(b"x"));
        assert!(DiskBlobStore::open(&path, false, 1 << 20).unwrap().has(5));
        let full = DiskBlobStore::open(&path, true, 1 << 20).unwrap();
        assert!(!full.has(5), "a physics bot must still ask for blobs an idle run only saw");
        full.insert(5, &Bytes::from_static(b"x"));
        assert_eq!(full.get(5).as_deref(), Some(&b"x"[..]));
        std::fs::remove_file(&path).unwrap();
    }
}
