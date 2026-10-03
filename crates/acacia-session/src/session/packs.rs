//! Resource packs during login. Packs the account's store lacks are requested (SendPacks), fetched
//! chunk by chunk, checked against the server's SHA-256 and recorded; then HaveAllPacks, the answer
//! a client gives straight away when it already holds everything. Like vanilla (2026-10-03 capture
//! against tools/packtest-setup.ps1's server), every chunk is requested at once on the data info.

use std::collections::HashMap;
use std::sync::Arc;

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

// BDS 1.26.52 accepts only `cancel`, `downloading`, `downloadingfinished` and
// `resourcepackstackfinished` here, and drops a client that sends anything else.
const SEND_PACKS_NAME: &str = "downloading";
const HAVE_ALL_NAME: &str = "downloadingfinished";
const COMPLETED_NAME: &str = "resourcepackstackfinished";

pub(super) struct Packs {
    store: Arc<dyn PackStore>,
    /// Requested packs whose download hasn't finished, by `<uuid>_<version>`.
    downloads: HashMap<String, Download>,
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
        Self { store, downloads: HashMap::new() }
    }
}

impl Session {
    pub(super) fn on_packs_info(&mut self, raw: &RawPacket) -> Result<(), Error> {
        let info: ResourcePacksInfo = raw.decode()?;
        let missing: Vec<String> =
            info.texture_packs.iter().map(|p| format!("{}_{}", p.uuid, p.version)).filter(|id| !self.packs.store.has(id)).collect();
        if missing.is_empty() {
            self.respond_to_packs(delay::PACKS_HAVE_ALL, Status::HaveAllPacks, HAVE_ALL_NAME, None);
            return Ok(());
        }
        tracing::debug!(?missing, "downloading resource packs");
        self.packs.downloads = missing.iter().map(|id| (id.clone(), Download::default())).collect();
        self.respond_to_packs(delay::PACKS_HAVE_ALL, Status::SendPacks, SEND_PACKS_NAME, Some(missing));
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
        if self.packs.downloads.is_empty() {
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use acacia_proto::packets::{ResourcePackDataInfoPackType, ResourcePacksInfoWorldTemplate};
    use acacia_proto::types::TexturePackInfosItem;
    use acacia_proto::{encode_packet, Packet};
    use bytes::BytesMut;
    use p384::ecdsa::SigningKey;

    use super::*;
    use crate::batch::BatchCodec;
    use crate::pack_store::MemoryPackStore;
    use crate::session::{LinkConfig, SessionConfig};

    const UUID: &str = "8fbd6e02-fb25-4e1d-adc2-776df9dfb30b";
    const ID: &str = "8fbd6e02-fb25-4e1d-adc2-776df9dfb30b_1.0.0";

    fn session(store: Arc<dyn PackStore>, now: Instant) -> Session {
        let cfg = SessionConfig {
            link: LinkConfig::Message,
            key: SigningKey::from_slice(&[7; 48]).unwrap(),
            login_request: Vec::new(),
            chunk_radius: 2,
            auto_respawn: false,
            initialize_on_spawn: true,
            blob_store: None,
            pack_store: store,
        };
        Session::new(cfg, "127.0.0.1:19132".parse().unwrap(), now)
    }

    fn feed<T: Packet>(s: &mut Session, now: Instant, packet: &T) {
        let mut buf = BytesMut::new();
        encode_packet(packet, &mut buf);
        s.handle_message(now, BatchCodec::without_header().encode([&buf[..]]));
    }

    /// Every packet the session sends by `at`, each with its header.
    fn sent(s: &mut Session, at: Instant) -> Vec<RawPacket> {
        s.handle_timeout(at);
        let mut packets = Vec::new();
        while let Some(msg) = s.poll_transmit(at) {
            BatchCodec::without_header().decode(&msg, &mut packets).unwrap();
        }
        packets.into_iter().map(|p| RawPacket::parse(p).unwrap()).collect()
    }

    fn packs_info() -> ResourcePacksInfo {
        let pack = TexturePackInfosItem {
            uuid: UUID.parse().unwrap(),
            version: "1.0.0".into(),
            size: 6,
            content_key: String::new(),
            sub_pack_name: String::new(),
            content_identity: String::new(),
            has_scripts: false,
            addon_pack: false,
            rtx_enabled: false,
            cdn_url: String::new(),
        };
        ResourcePacksInfo {
            must_accept: true,
            has_addons: false,
            has_scripts: false,
            disable_vibrant_visuals: false,
            world_template: ResourcePacksInfoWorldTemplate { uuid: UUID.parse().unwrap(), version: String::new() },
            texture_packs: vec![pack],
        }
    }

    fn response(packets: &[RawPacket]) -> ResourcePackClientResponse {
        packets.iter().find(|p| p.id == ResourcePackClientResponse::ID).expect("a pack response").decode().unwrap()
    }

    #[test]
    fn downloads_missing_packs_once_per_store() {
        let store: Arc<dyn PackStore> = Arc::new(MemoryPackStore::default());
        let mut now = Instant::now();
        let mut s = session(store.clone(), now);
        sent(&mut s, now);
        let step = Duration::from_secs(1);

        feed(&mut s, now, &packs_info());
        now += step;
        let asked = response(&sent(&mut s, now));
        assert_eq!((asked.response_status, asked.resourcepackids), (Status::SendPacks, Some(vec![ID.to_owned()])));

        let payload = [b"abcd".as_slice(), b"ef"];
        let hash = Sha256::digest(payload.concat());
        let info = ResourcePackDataInfo {
            pack_id: ID.into(),
            max_chunk_size: 4,
            chunk_count: 2,
            size: 6,
            hash: Bytes::copy_from_slice(&hash),
            is_premium: false,
            pack_type: ResourcePackDataInfoPackType::Resources,
        };
        feed(&mut s, now, &info);
        let requests: Vec<u32> =
            sent(&mut s, now).iter().map(|p| p.decode::<ResourcePackChunkRequest>().unwrap().chunk_index).collect();
        assert_eq!(requests, [0, 1]);

        for (index, progress) in [(1u32, 4u64), (0, 0)] {
            let data = Bytes::from_static(payload[index as usize]);
            feed(&mut s, now, &ResourcePackChunkData { pack_id: ID.into(), chunk_index: index, progress, payload: data });
        }
        now += step;
        assert_eq!(response(&sent(&mut s, now)).response_status, Status::HaveAllPacks);
        assert!(store.has(ID));

        let mut again = session(store, now);
        sent(&mut again, now);
        feed(&mut again, now, &packs_info());
        assert_eq!(response(&sent(&mut again, now + step)).response_status, Status::HaveAllPacks, "a returning account has it");
    }
}
