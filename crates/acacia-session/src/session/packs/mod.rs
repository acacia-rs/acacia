//! Resource packs during login. Packs the account's store lacks are fetched, then HaveAllPacks, the
//! answer a client gives straight away when it already holds everything. Like vanilla (2026-10-03
//! captures against tools/packtest-setup.ps1's server):
//! - a pack with a `cdn_url` is downloaded over HTTP by the I/O driver (`poll_pack_fetch`) and
//!   never requested from the server;
//! - the rest are asked for (SendPacks) and every chunk is requested at once on the data info, then
//!   checked against the server's SHA-256.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use acacia_proto::packets::{
    ResourcePackChunkData, ResourcePackChunkRequest, ResourcePackClientResponse, ResourcePackClientResponseResponseStatus as Status,
    ResourcePackDataInfo, ResourcePacksInfo,
};
use acacia_proto::RawPacket;
use bytes::Bytes;
use sha2::{Digest, Sha256};

use super::deferred::delay;
use super::Session;
use crate::pack_store::PackStore;
use crate::Error;

#[cfg(test)]
mod tests;

// BDS 1.26.52 accepts only `cancel`, `downloading`, `downloadingfinished` and
// `resourcepackstackfinished` here, and drops a client that sends anything else.
const SEND_PACKS_NAME: &str = "downloading";
const HAVE_ALL_NAME: &str = "downloadingfinished";
const COMPLETED_NAME: &str = "resourcepackstackfinished";

/// A pack to download over HTTP: `HEAD` then `GET` of exactly `url`, like vanilla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFetch {
    /// `<uuid>_<version>`; hand it back to [`Session::pack_fetched`].
    pub id: String,
    pub url: String,
}

pub(super) struct Packs {
    store: Arc<dyn PackStore>,
    /// Requested packs whose download hasn't finished, by `<uuid>_<version>`.
    downloads: HashMap<String, Download>,
    /// HTTP packs not yet handed to the driver.
    to_fetch: VecDeque<PackFetch>,
    /// HTTP packs the driver hasn't reported back.
    fetching: HashSet<String>,
}

#[derive(Default)]
struct Download {
    /// From the pack's ResourcePackDataInfo; until then no chunk is expected.
    expected_hash: Option<Bytes>,
    chunks: Vec<Option<Bytes>>,
}

impl Download {
    fn is_complete(&self) -> bool {
        self.expected_hash.is_some() && self.chunks.iter().all(Option::is_some)
    }
}

impl Packs {
    pub fn new(store: Arc<dyn PackStore>) -> Self {
        Self { store, downloads: HashMap::new(), to_fetch: VecDeque::new(), fetching: HashSet::new() }
    }

    fn is_done(&self) -> bool {
        self.downloads.is_empty() && self.fetching.is_empty()
    }
}

impl Session {
    /// The next pack the I/O driver should download over HTTP, reporting it with
    /// [`pack_fetched`](Self::pack_fetched).
    pub fn poll_pack_fetch(&mut self) -> Option<PackFetch> {
        self.packs.to_fetch.pop_front()
    }

    /// Records the outcome of a [`PackFetch`]; the last one outstanding sends HaveAllPacks.
    pub fn pack_fetched(&mut self, now: Instant, id: &str, ok: bool) {
        if !self.packs.fetching.remove(id) {
            return;
        }
        self.now = now;
        if ok {
            self.packs.store.insert(id);
        } else {
            // Vanilla's reaction to a failed download is uncaptured; carry on uncached.
            tracing::warn!(pack = id, "resource pack download failed");
        }
        if self.packs.is_done() {
            self.respond_to_packs(delay::PACKS_FETCHED, Status::HaveAllPacks, HAVE_ALL_NAME, None);
        }
    }

    pub(super) fn on_packs_info(&mut self, raw: &RawPacket) -> Result<(), Error> {
        let info: ResourcePacksInfo = raw.decode()?;
        let missing = info.texture_packs.iter().map(|p| (format!("{}_{}", p.uuid, p.version), &p.cdn_url)).filter(|(id, _)| !self.packs.store.has(id));
        let (by_url, by_chunk): (Vec<_>, Vec<_>) = missing.partition(|(_, url)| !url.is_empty());
        for (id, url) in by_url {
            self.packs.fetching.insert(id.clone());
            self.packs.to_fetch.push_back(PackFetch { id, url: url.clone() });
        }
        if by_chunk.is_empty() {
            if self.packs.is_done() {
                self.respond_to_packs(delay::PACKS_HAVE_ALL, Status::HaveAllPacks, HAVE_ALL_NAME, None);
            }
            return Ok(());
        }
        let ids: Vec<String> = by_chunk.into_iter().map(|(id, _)| id).collect();
        tracing::debug!(?ids, "downloading resource packs");
        self.packs.downloads = ids.iter().map(|id| (id.clone(), Download::default())).collect();
        self.respond_to_packs(delay::PACKS_HAVE_ALL, Status::SendPacks, SEND_PACKS_NAME, Some(ids));
        Ok(())
    }

    pub(super) fn on_pack_data_info(&mut self, raw: &RawPacket) -> Result<(), Error> {
        let info: ResourcePackDataInfo = raw.decode()?;
        let Some(download) = self.packs.downloads.get_mut(&info.pack_id) else {
            tracing::debug!(pack = info.pack_id, "data info for a pack we did not ask for");
            return Ok(());
        };
        download.expected_hash = Some(info.hash);
        download.chunks = vec![None; info.chunk_count as usize];
        for chunk_index in 0..info.chunk_count {
            self.send(&ResourcePackChunkRequest { pack_id: info.pack_id.clone(), chunk_index });
        }
        Ok(())
    }

    pub(super) fn on_pack_chunk(&mut self, raw: &RawPacket) -> Result<(), Error> {
        let chunk: ResourcePackChunkData = raw.decode()?;
        let Some(download) = self.packs.downloads.get_mut(&chunk.pack_id) else { return Ok(()) };
        if let Some(slot) = download.chunks.get_mut(chunk.chunk_index as usize) {
            *slot = Some(chunk.payload);
        }
        if !download.is_complete() {
            return Ok(());
        }
        let download = self.packs.downloads.remove(&chunk.pack_id).expect("found above");
        let mut hash = Sha256::new();
        download.chunks.iter().flatten().for_each(|c| hash.update(c));
        if download.expected_hash.as_deref() == Some(&hash.finalize()[..]) {
            self.packs.store.insert(&chunk.pack_id);
        } else {
            tracing::warn!(pack = chunk.pack_id, "resource pack hash mismatch; not cached");
        }
        if self.packs.is_done() {
            self.respond_to_packs(delay::PACKS_DOWNLOADED, Status::HaveAllPacks, HAVE_ALL_NAME, None);
        }
        Ok(())
    }

    pub(super) fn on_pack_stack(&mut self) {
        self.respond_to_packs(delay::PACKS_COMPLETED, Status::Completed, COMPLETED_NAME, None);
    }

    fn respond_to_packs(&mut self, delay_ms: (u64, u64), status: Status, name: &str, ids: Option<Vec<String>>) {
        let response = ResourcePackClientResponse { response_status: status, response_status_name: name.into(), resourcepackids: ids };
        self.send_later(delay_ms, &response);
    }
}
