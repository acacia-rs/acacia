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
const STEP: Duration = Duration::from_secs(1);

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
        strict: false,
    };
    let mut s = Session::new(cfg, "127.0.0.1:19132".parse().unwrap(), now);
    sent(&mut s, now);
    s
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

fn packs_info(cdn_url: &str) -> ResourcePacksInfo {
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
        cdn_url: cdn_url.into(),
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

fn response(packets: &[RawPacket]) -> Option<ResourcePackClientResponse> {
    packets.iter().find(|p| p.id == ResourcePackClientResponse::ID).map(|p| p.decode().unwrap())
}

fn have_all(packets: &[RawPacket]) -> bool {
    response(packets).is_some_and(|r| r.response_status == Status::HaveAllPacks)
}

#[test]
fn downloads_missing_packs_once_per_store() {
    downloads_a_pack_the_server_names(ID);
}

#[test]
fn downloads_a_pack_the_server_names_by_uuid_alone() {
    downloads_a_pack_the_server_names(UUID);
}

fn downloads_a_pack_the_server_names(pack_id: &str) {
    let store: Arc<dyn PackStore> = Arc::new(MemoryPackStore::default());
    let mut now = Instant::now();
    let mut s = session(store.clone(), now);

    feed(&mut s, now, &packs_info(""));
    now += STEP;
    let asked = response(&sent(&mut s, now)).unwrap();
    assert_eq!((asked.response_status, asked.resourcepackids), (Status::SendPacks, Some(vec![ID.to_owned()])));

    let payload = [b"abcd".as_slice(), b"ef"];
    let hash = Sha256::digest(payload.concat());
    let info = ResourcePackDataInfo {
        pack_id: pack_id.into(),
        max_chunk_size: 4,
        chunk_count: 2,
        size: 6,
        hash: Bytes::copy_from_slice(&hash),
        is_premium: false,
        pack_type: ResourcePackDataInfoPackType::Resources,
    };
    feed(&mut s, now, &info);
    let requests: Vec<u32> = sent(&mut s, now).iter().map(|p| p.decode::<ResourcePackChunkRequest>().unwrap().chunk_index).collect();
    assert_eq!(requests, [0, 1]);

    for (index, progress) in [(1u32, 4u64), (0, 0)] {
        let data = Bytes::from_static(payload[index as usize]);
        feed(&mut s, now, &ResourcePackChunkData { pack_id: pack_id.into(), chunk_index: index, progress, payload: data });
    }
    now += STEP;
    assert!(have_all(&sent(&mut s, now)));
    assert!(store.has(ID));

    let mut again = session(store, now);
    feed(&mut again, now, &packs_info(""));
    assert!(have_all(&sent(&mut again, now + STEP)), "a returning account has it");
}

#[test]
fn url_packs_are_fetched_over_http_not_requested() {
    let store: Arc<dyn PackStore> = Arc::new(MemoryPackStore::default());
    let mut now = Instant::now();
    let mut s = session(store.clone(), now);
    let url = "https://cdn.example/pack.zip";

    feed(&mut s, now, &packs_info(url));
    assert_eq!(s.poll_pack_fetch(), Some(PackFetch { id: ID.into(), url: url.into() }));
    assert_eq!(s.poll_pack_fetch(), None);
    now += Duration::from_secs(10);
    assert_eq!(response(&sent(&mut s, now)), None, "nothing until the download is done");

    s.pack_fetched(now, ID, true);
    assert!(!have_all(&sent(&mut s, now + Duration::from_millis(delay::PACKS_FETCHED.0 - 1))));
    assert!(have_all(&sent(&mut s, now + Duration::from_millis(delay::PACKS_FETCHED.1))));
    assert!(store.has(ID));

    let mut again = session(store, now);
    feed(&mut again, now, &packs_info(url));
    assert_eq!(again.poll_pack_fetch(), None, "a returning account has it");
}
