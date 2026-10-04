//! Terrain tracking for physics: builds the block registry from StartGame, feeds chunks into a
//! [`ChunkView`], and requests sub-chunks from servers that use request mode (BDS). Idle bots keep
//! only the columns around them ([`WorldTracker::nearby`]).

mod adapter;
mod blobs;
mod near;
#[cfg(test)]
mod tests;

pub use adapter::PhysicsWorld;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use acacia_client::proto::nbt::Value;
use acacia_client::BlobStore;
use acacia_client::proto::packets::{
    ChangeDimension, ClientCacheMissResponse, LevelChunk, NetworkChunkPublisherUpdate, StartGame, Subchunk, SubchunkRequest, UpdateBlock,
    UpdateBlockSynced, UpdateSubchunkBlocks,
};
use acacia_client::proto::types::{BlockCoordinates, SubChunkEntryItemResult, Vec3i8, Vec3li};
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use acacia_world::{BlockIds, BlockRegistry, ChunkView, CustomBlock, Dimension, World};
use blobs::{Blobs, Ready};
use bytes::Bytes;
use near::Near;

/// (server address, dimension id) → world.
type WorldMap = HashMap<(String, i32), Weak<World>>;

/// Worlds shared by every bot connected to the same server, so each chunk is stored once.
#[derive(Clone, Default)]
pub struct SharedWorlds(Arc<Mutex<WorldMap>>);

impl SharedWorlds {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_or_create(&self, key: (String, i32), make: impl FnOnce() -> Arc<World>) -> Arc<World> {
        let mut worlds = self.0.lock().expect("shared worlds lock poisoned");
        if let Some(world) = worlds.get(&key).and_then(Weak::upgrade) {
            return world;
        }
        worlds.retain(|_, w| w.strong_count() > 0);
        let world = make();
        worlds.insert(key, Arc::downgrade(&world));
        world
    }
}

pub struct WorldTracker {
    server: String,
    shared: SharedWorlds,
    registry: Option<Arc<BlockRegistry>>,
    ids: BlockIds,
    view: Option<ChunkView>,
    blobs: Blobs,
    /// Packets to send on the tracker's behalf (sub-chunk requests).
    pub(crate) outgoing: Vec<SubchunkRequest>,
    near: Option<Near>,
}

impl WorldTracker {
    pub const PACKETS: &'static [u32] = &[
        StartGame::ID,
        LevelChunk::ID,
        Subchunk::ID,
        UpdateBlock::ID,
        UpdateBlockSynced::ID,
        UpdateSubchunkBlocks::ID,
        ChangeDimension::ID,
        NetworkChunkPublisherUpdate::ID,
        ClientCacheMissResponse::ID,
    ];

    pub fn new(server: String, shared: SharedWorlds) -> Self {
        Self { server, shared, registry: None, ids: BlockIds::Runtime, view: None, blobs: Blobs::default(), outgoing: Vec::new(), near: None }
    }

    /// Idle bots: requests nothing (crate::subchunks does, like vanilla) and decodes only the columns
    /// around the player, as last given to [`WorldTracker::follow`] (near.rs).
    pub(crate) fn nearby(self) -> Self {
        Self { near: Some(Near::default()), blobs: Blobs::remembering(near::RECENT_BLOBS), ..self }
    }

    /// Nearby mode: centres the kept columns on the player's feet, dropping those left behind.
    pub(crate) fn follow(&mut self, x: f32, z: f32) {
        let (cx, cz) = ((x.floor() as i32) >> 4, (z.floor() as i32) >> 4);
        let Some(near) = &mut self.near else { return };
        if near.recenter(cx, cz) {
            if let Some(view) = &mut self.view {
                view.retain_within(cx, cz, near::RADIUS);
            }
            self.blobs.retain_waiting(|(x, _, z)| near.covers(x, z));
        }
    }

    /// Whether the block at a position is known: its chunk is loaded and, in nearby mode, its section arrived.
    pub fn knows_block(&self, x: i32, y: i32, z: i32) -> bool {
        self.view.as_ref().is_some_and(|v| v.chunk(x >> 4, z >> 4).is_some())
            && self.near.as_ref().is_none_or(|n| n.knows(x >> 4, y >> 4, z >> 4))
    }

    /// The connection's blob store, for sections whose blobs it already held (blobs.rs).
    pub fn set_blob_store(&mut self, store: Arc<dyn BlobStore>) {
        self.blobs.set_store(store);
    }

    pub fn view(&self) -> Option<&ChunkView> {
        self.view.as_ref()
    }

    pub fn registry(&self) -> Option<&Arc<BlockRegistry>> {
        self.registry.as_ref()
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            StartGame::ID => {
                let sg: StartGame = packet.decode()?;
                let custom = custom_blocks(&sg);
                self.registry = Some(if custom.is_empty() {
                    BlockRegistry::vanilla_arc()
                } else {
                    Arc::new(BlockRegistry::vanilla().with_custom_blocks(&custom))
                });
                self.ids = if sg.block_network_ids_are_hashes { BlockIds::Hashed } else { BlockIds::Runtime };
                self.enter_dimension(sg.dimension.to_raw() as i32);
            }
            ChangeDimension::ID => self.enter_dimension(packet.decode::<ChangeDimension>()?.dimension),
            LevelChunk::ID => self.level_chunk(packet.decode()?),
            Subchunk::ID => {
                let sc: Subchunk = packet.decode()?;
                tracing::trace!(origin = ?sc.origin, entries = sc.entries.len(), results = ?sc.entries.iter().map(|e| e.result).collect::<Vec<_>>(), "sub-chunk response");
                let Some(view) = &mut self.view else { return Ok(()) };
                for e in sc.entries {
                    let pos = (sc.origin.x + i32::from(e.dx), sc.origin.y + i32::from(e.dy), sc.origin.z + i32::from(e.dz));
                    if self.near.as_ref().is_some_and(|n| !n.covers(pos.0, pos.2)) {
                        continue;
                    }
                    if e.result == SubChunkEntryItemResult::SuccessAllAir {
                        view.insert_sub_chunk_air(pos.0, pos.1, pos.2);
                        if let Some(near) = &mut self.near {
                            near.mark(pos.0, pos.1, pos.2);
                        }
                        continue;
                    }
                    if e.result != SubChunkEntryItemResult::Success {
                        continue;
                    }
                    let payload = e.payload.unwrap_or_default();
                    let section = match (sc.cache_enabled, e.blob_id) {
                        (true, Some(id)) => self.blobs.section(pos, id, payload),
                        _ => Some(payload),
                    };
                    if let Some(section) = section {
                        insert_section(view, self.near.as_mut(), pos, &section);
                    }
                }
            }
            ClientCacheMissResponse::ID => {
                let Some(view) = &mut self.view else { return Ok(()) };
                for blob in packet.decode::<ClientCacheMissResponse>()?.blobs {
                    for ready in self.blobs.delivered(blob.hash, &blob.payload) {
                        match ready {
                            Ready::Section(pos, section) => insert_section(view, self.near.as_mut(), pos, &section),
                            Ready::Biomes((x, z), biomes) => view.insert_biomes(x, z, &biomes),
                        }
                    }
                }
            }
            UpdateBlock::ID => {
                let u: UpdateBlock = packet.decode()?;
                self.set_block(u.position, u.layer, u.block_runtime_id);
            }
            UpdateBlockSynced::ID => {
                let u: UpdateBlockSynced = packet.decode()?;
                self.set_block(u.position, u.layer, u.block_runtime_id);
            }
            UpdateSubchunkBlocks::ID => {
                let u: UpdateSubchunkBlocks = packet.decode()?;
                for (layer, list) in [(0, u.blocks), (1, u.extra)] {
                    for b in list {
                        self.set_block(b.position, layer, b.runtime_id);
                    }
                }
            }
            NetworkChunkPublisherUpdate::ID => {
                let p: NetworkChunkPublisherUpdate = packet.decode()?;
                if let Some(view) = &mut self.view {
                    // +1 so chunks at the edge of the publisher radius are not dropped and re-sent.
                    view.retain_within(p.coordinates.x >> 4, p.coordinates.z >> 4, (p.radius as i32 >> 4) + 1);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn set_block(&self, p: BlockCoordinates, layer: u32, wire_id: u32) {
        let Some(view) = &self.view else { return };
        let known = view.set_block(p.x, p.y, p.z, layer, wire_id);
        tracing::trace!(x = p.x, y = p.y, z = p.z, layer, wire_id, known, "block update");
    }

    fn enter_dimension(&mut self, dimension: i32) {
        let Some(registry) = &self.registry else { return };
        let (registry, ids) = (registry.clone(), self.ids);
        let world = self.shared.get_or_create((self.server.clone(), dimension), || World::new(registry, dimension, ids));
        self.view = Some(ChunkView::new(world));
        self.blobs.clear();
        if let Some(near) = &mut self.near {
            near.forget();
        }
    }

    fn level_chunk(&mut self, c: LevelChunk) {
        let Some(view) = &mut self.view else { return };
        if self.near.as_ref().is_some_and(|n| !n.covers(c.x, c.z)) {
            return;
        }
        // Since 26.50 a sub-chunk limit (instead of magic counts) marks request mode; negative = no limit.
        match c.highest_subchunk_count {
            Some(_) if self.near.is_some() => {}
            Some(limit) => {
                // The payload is the column's biomes, or with the cache its only blob is.
                let biomes = match (c.cache_enabled, c.blobs.first()) {
                    (false, _) => Some(c.payload.clone()),
                    (true, Some(&id)) => self.blobs.biomes((c.x, c.z), id),
                    (true, None) => None,
                };
                if let Some(biomes) = biomes {
                    view.insert_biomes(c.x, c.z, &biomes);
                }
                let dim = Dimension::from_id(c.dimension, 0);
                let lowest = dim.min_y >> 4;
                let full = (dim.height >> 4) as i32;
                let count = if limit < 0 { full } else { limit.min(full) };
                view.insert_sub_chunk_limit(c.x, c.z, count as usize);
                let requests = (0..count).map(|i| Vec3i8 { x: 0, y: (lowest + i) as i8, z: 0 }).collect();
                self.outgoing.push(SubchunkRequest { dimension: c.dimension, requests, origin: Vec3li { x: c.x, y: 0, z: c.z } });
            }
            // Layout from gophertunnel (unverified on BDS, which uses request mode): one blob per section,
            // bottom first, then the biome blob; the payload is border blocks + block entities.
            None if c.cache_enabled => {
                let reset = view.insert_level_chunk(c.x, c.z, 0, &[]).map(drop);
                log_err(reset);
                let lowest = Dimension::from_id(c.dimension, 0).min_y >> 4;
                let sections = c.blobs.len().min(c.sub_chunk_count as usize);
                for (i, &id) in c.blobs[..sections].iter().enumerate() {
                    let pos = (c.x, lowest + i as i32, c.z);
                    if let Some(section) = self.blobs.section(pos, id, Bytes::new()) {
                        insert_section(view, self.near.as_mut(), pos, &section);
                    }
                }
                if let Some(biomes) = c.blobs.get(sections).and_then(|&id| self.blobs.biomes((c.x, c.z), id)) {
                    view.insert_biomes(c.x, c.z, &biomes);
                }
            }
            None => {
                let inserted = view.insert_level_chunk(c.x, c.z, c.sub_chunk_count, &c.payload).map(drop);
                if let (Ok(()), Some(near)) = (&inserted, &mut self.near) {
                    let dim = Dimension::from_id(c.dimension, 0);
                    ((dim.min_y >> 4)..((dim.min_y + dim.height as i32) >> 4)).for_each(|y| near.mark(c.x, y, c.z));
                }
                log_err(inserted);
            }
        }
    }
}

fn insert_section(view: &mut ChunkView, near: Option<&mut Near>, (x, y, z): (i32, i32, i32), section: &[u8]) {
    let inserted = view.insert_sub_chunk(x, y, z, section);
    if let (Ok(()), Some(near)) = (&inserted, near) {
        near.mark(x, y, z);
    }
    log_err(inserted);
}

/// Whether the chunk at `feet` is loaded and a solid block lies within 3 blocks below it.
pub(crate) fn is_chunk_loaded(view: &ChunkView, feet: [f32; 3]) -> bool {
    view.chunk((feet[0].floor() as i32) >> 4, (feet[2].floor() as i32) >> 4).is_some()
}

/// Solid ground or liquid within three blocks below the feet.
pub(crate) fn has_support_below(view: &ChunkView, registry: &BlockRegistry, feet: [f32; 3]) -> bool {
    use acacia_world::BlockAccess;
    let (x, y, z) = (feet[0].floor() as i32, feet[1].floor() as i32, feet[2].floor() as i32);
    (y - 3..=y).any(|y| registry.get(view.block(x, y, z)).is_some_and(|s| s.is_solid() || s.is_water() || s.is_lava()))
}

fn log_err(r: Result<(), acacia_world::Error>) {
    if let Err(e) = r {
        tracing::debug!(error = %e, "chunk data rejected");
    }
}

/// The id air has in block updates, without building the registry.
pub(crate) fn air_wire_id(sg: &StartGame) -> u32 {
    let vanilla = BlockRegistry::vanilla();
    if sg.block_network_ids_are_hashes {
        return vanilla.get(vanilla.air_id()).map_or(0, |s| s.network_hash);
    }
    vanilla.air_id_with_custom_blocks(&custom_blocks(sg))
}

/// Custom block state counts: the product of each property's enum length, as the client permutes them.
fn custom_blocks(sg: &StartGame) -> Vec<CustomBlock> {
    let vanilla = BlockRegistry::vanilla();
    let enum_len = |p: &Value| match p.get("enum") {
        Some(Value::List(e)) => e.items.len().max(1) as u32,
        _ => 1,
    };
    sg.block_properties
        .iter()
        .filter(|b| vanilla.states_of(&b.name).next().is_none())
        .map(|b| {
            let state_count = match b.state.value.get("properties") {
                Some(Value::List(l)) => l.items.iter().map(enum_len).product(),
                _ => 1,
            };
            CustomBlock { name: b.name.clone(), state_count }
        })
        .collect()
}
