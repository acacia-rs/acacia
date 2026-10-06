//! Extracts real NBT documents for acacia-nbt's tests, benches and fuzz seeds from the recorded BDS
//! join and the captured chunk fixtures (crates/acacia-nbt/README.md, "Corpus").
//! `cargo run -p acacia-nbtcorpus`

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::{fs, io};

use acacia_proto::nbt::{self, LittleEndian, Nbt, Network, Raw};
use acacia_proto::packets::{
    AvailableEntityIdentifiers, BlockEntityData, CreativeContent, InventoryContent, InventorySlot, ItemRegistry,
    LevelChunk, StartGame, SyncEntityProperty,
};
use acacia_proto::types::{ItemLegacy, ItemLegacyExtra, ItemV4, ItemV4Extra};
use acacia_proto::{Packet, RawPacket, manual};
use acacia_testserver::{Script, Step};
use acacia_world::{Dimension, level_chunk_block_entities};
use bytes::{Bytes, BytesMut};

const WORLD_FIXTURES: &str = "crates/acacia-world/tests/fixtures";
const OUT: &str = "crates/acacia-nbt/corpus";
/// Documents kept per kind, spread over the sizes seen.
const PER_KIND: usize = 24;

/// Distinct encoded documents by (flavour directory, kind).
#[derive(Default)]
struct Corpus(BTreeMap<(&'static str, &'static str), BTreeSet<Vec<u8>>>);

impl Corpus {
    fn network(&mut self, kind: &'static str, raw: &Raw<Network>) {
        if !raw.is_end() {
            self.0.entry(("network", kind)).or_default().insert(raw.as_bytes().to_vec());
        }
    }

    fn item(&mut self, nbt: Option<&Nbt>) {
        if let Some(nbt) = nbt {
            let mut w = BytesMut::new();
            nbt::write::<LittleEndian>(&mut w, nbt);
            self.0.entry(("le", "item")).or_default().insert(w.to_vec());
        }
    }

    fn packet(&mut self, raw: &RawPacket) {
        if let Some(p) = decode::<StartGame>(raw) {
            p.block_properties.iter().for_each(|b| self.network("block-state", &b.state));
            self.network("properties", &p.property_data);
        } else if let Some(p) = decode::<ItemRegistry>(raw) {
            // Items after this decode against the registry's shield id (docs/proto.md, "Known gaps").
            if let Some(shield) = p.itemstates.iter().find(|i| i.name == "minecraft:shield") {
                manual::set_shield_item_id(shield.runtime_id.into());
            }
            p.itemstates.iter().for_each(|i| self.network("item-component", &i.nbt));
        } else if let Some(p) = decode::<AvailableEntityIdentifiers>(raw) {
            self.network("entity-ids", &p.nbt);
        } else if let Some(p) = decode::<SyncEntityProperty>(raw) {
            self.network("entity-property", &p.nbt);
        } else if let Some(p) = decode::<BlockEntityData>(raw) {
            self.network("block-entity", &p.nbt);
        } else if let Some(p) = decode::<CreativeContent>(raw) {
            p.items.iter().for_each(|i| self.item(legacy_nbt(&i.item)));
        } else if let Some(p) = decode::<InventoryContent>(raw) {
            p.input.iter().for_each(|i| self.item(v4_nbt(i)));
        } else if let Some(p) = decode::<InventorySlot>(raw) {
            self.item(v4_nbt(&p.item));
        } else if let Some(p) = decode::<LevelChunk>(raw) {
            self.level_chunk(&p);
        }
    }

    /// Same offsets as acacia-bot's `BlockEntities::level_chunk`.
    fn level_chunk(&mut self, c: &LevelChunk) {
        let request_mode = c.highest_subchunk_count.is_some();
        let sections = if request_mode || c.cache_enabled { 0 } else { c.sub_chunk_count };
        let biomes = (!c.cache_enabled).then(|| Dimension::from_id(c.dimension, 0));
        let Ok(offset) = level_chunk_block_entities(&c.payload, sections, biomes) else {
            return;
        };
        let mut tail = &c.payload[offset..];
        while !tail.is_empty() {
            match nbt::read::<Network>(&mut tail) {
                Ok(nbt) => self.network("block-entity", &(&nbt).into()),
                Err(_) => break,
            }
        }
    }

    fn write(&self, out: &Path) -> io::Result<()> {
        for dir in ["network", "le"] {
            let dir = out.join(dir);
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
            }
            fs::create_dir_all(&dir)?;
        }
        for ((dir, kind), docs) in &self.0 {
            let mut docs: Vec<_> = docs.iter().collect();
            docs.sort_by_key(|d| d.len());
            let kept = sample(&docs);
            for (i, doc) in kept.iter().enumerate() {
                fs::write(out.join(dir).join(format!("{kind}-{i:02}.nbt")), doc)?;
            }
            let bytes: usize = kept.iter().map(|d| d.len()).sum();
            println!("{dir}/{kind}: {} of {} documents, {bytes} bytes", kept.len(), docs.len());
        }
        Ok(())
    }
}

fn sample<'a>(sorted: &[&'a Vec<u8>]) -> Vec<&'a Vec<u8>> {
    if sorted.len() <= PER_KIND {
        return sorted.to_vec();
    }
    (0..PER_KIND).map(|i| sorted[i * (sorted.len() - 1) / (PER_KIND - 1)]).collect()
}

fn decode<T: Packet>(raw: &RawPacket) -> Option<T> {
    if !raw.is::<T>() {
        return None;
    }
    raw.decode().inspect_err(|e| eprintln!("skipped: {e}")).ok()
}

fn v4_nbt(item: &ItemV4) -> Option<&Nbt> {
    match &item.extra {
        ItemV4Extra::ShieldItemID(e) => e.as_ref()?.nbt.as_ref().map(|n| &n.nbt),
        ItemV4Extra::Default(e) => e.as_ref()?.nbt.as_ref().map(|n| &n.nbt),
    }
}

fn legacy_nbt(item: &ItemLegacy) -> Option<&Nbt> {
    match &item.extra {
        ItemLegacyExtra::ShieldItemID(e) => e.as_ref()?.nbt.as_ref().map(|n| &n.nbt),
        ItemLegacyExtra::Default(e) => e.as_ref()?.nbt.as_ref().map(|n| &n.nbt),
    }
}

fn main() -> io::Result<()> {
    let mut corpus = Corpus::default();
    for step in Script::bds_spawn().steps {
        if let Step::Send { packet, .. } = step
            && let Ok(raw) = RawPacket::parse(packet)
        {
            corpus.packet(&raw);
        }
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for server in ["bds", "geyser"] {
        for entry in fs::read_dir(workspace.join(WORLD_FIXTURES).join(server))? {
            if let Ok(raw) = RawPacket::parse(Bytes::from(fs::read(entry?.path())?)) {
                corpus.packet(&raw);
            }
        }
    }
    corpus.write(&workspace.join(OUT))
}
