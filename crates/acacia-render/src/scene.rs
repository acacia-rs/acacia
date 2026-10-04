//! Tracks which sections need meshing or new light from the light thread's events and feeds the
//! worker pool, nearest first. Knows nothing about the GPU: it yields [`Update`]s for the renderer
//! to apply. One job per section is in flight at a time, so results arrive in request order.

use std::sync::Arc;

use acacia_world::{ChunkChange, World};
use glam::IVec3;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::biome::BiomeColors;
use crate::blocks::BlockTable;
use crate::light::{LightData, LightEvent, LightVolume, Lighting};
use parking_lot::RwLock;
use crate::mesh::SectionMesh;
use crate::workers::{Job, Output, SectionKey, Work, Workers};

pub enum Update {
    Mesh(SectionKey, SectionMesh),
    Light(SectionKey, LightVolume),
    Remove(SectionKey),
}

/// Frames between checks for columns every view has dropped.
const UNLOAD_CHECK_FRAMES: u64 = 30;
/// Sorting weight of each unloaded neighbour column: such a section will likely need remeshing
/// once the neighbour arrives, so it waits behind complete ones.
const MISSING_NEIGHBOUR_COST: i32 = 4096;

pub struct Scene {
    workers: Workers,
    table: Arc<BlockTable>,
    lighting: Lighting,
    tracked: Tracked,
    frame: u64,
}

struct Tracked {
    world: Arc<World>,
    columns: FxHashSet<(i32, i32)>,
    dirty: FxHashSet<SectionKey>,
    /// Sections whose light changed while their blocks didn't.
    relight: FxHashSet<SectionKey>,
    in_flight: FxHashSet<SectionKey>,
    /// Newest requested mesh version per section; results of older jobs are dropped.
    versions: FxHashMap<SectionKey, u64>,
    next_version: u64,
    sections: std::ops::Range<i32>,
    /// Face pairs per meshed section, for [`crate::cull`].
    visibility: FxHashMap<SectionKey, u16>,
}

impl Scene {
    /// `lighting` may come from a previous scene of the same world ([`Scene::into_parts`]): its
    /// lit columns are meshed at once.
    pub fn new(world: Arc<World>, table: Arc<BlockTable>, biomes: Arc<BiomeColors>, lighting: Lighting) -> Self {
        let dim = world.dimension();
        let min = dim.min_y >> 4;
        let mut t = Tracked {
            world: world.clone(),
            columns: FxHashSet::default(),
            dirty: FxHashSet::default(),
            relight: FxHashSet::default(),
            in_flight: FxHashSet::default(),
            versions: FxHashMap::default(),
            next_version: 1,
            sections: min..min + (dim.height / 16) as i32,
            visibility: FxHashMap::default(),
        };
        let lit: Vec<_> = lighting.data.read().columns.iter().copied().collect();
        for (x, z) in lit {
            t.apply(ChunkChange::Column { x, z });
        }
        let workers = Workers::new(world, lighting.data.clone(), table.clone(), biomes);
        Scene { workers, table, lighting, tracked: t, frame: 0 }
    }

    pub fn world(&self) -> &Arc<World> {
        &self.tracked.world
    }

    /// The world, block table and lighting, to build a replacement scene from.
    pub fn into_parts(self) -> (Arc<World>, Arc<BlockTable>, Lighting) {
        (self.tracked.world, self.table, self.lighting)
    }

    /// Light of the loaded columns, for [`crate::light::LightData::light`] lookups.
    pub fn light(&self) -> &Arc<RwLock<LightData>> {
        &self.lighting.data
    }

    pub fn pending(&self) -> usize {
        self.tracked.dirty.len() + self.tracked.in_flight.len()
    }

    pub fn sections(&self) -> std::ops::Range<i32> {
        self.tracked.sections.clone()
    }

    /// A section's face pairs: all open until meshed, `None` outside loaded columns.
    pub fn visibility(&self, key: SectionKey) -> Option<u16> {
        let t = &self.tracked;
        (t.sections.contains(&key.1) && t.columns.contains(&(key.0, key.2)))
            .then(|| t.visibility.get(&key).copied().unwrap_or(crate::mesh::visibility::ALL))
    }

    pub fn pump(&mut self, camera_block: IVec3, out: &mut Vec<Update>) {
        self.frame += 1;
        let t = &mut self.tracked;
        for event in self.lighting.events.try_iter() {
            match event {
                LightEvent::World(change) => t.apply(change),
                LightEvent::Light(key) => t.light_changed(key),
            }
        }
        if self.frame.is_multiple_of(UNLOAD_CHECK_FRAMES) {
            t.drop_unloaded(out);
        }
        for done in self.workers.finished() {
            t.in_flight.remove(&done.key);
            match done.output {
                Output::Mesh { version, mesh } => {
                    if t.versions.get(&done.key) != Some(&version) {
                        continue;
                    }
                    t.versions.remove(&done.key);
                    out.push(match mesh {
                        Some(mesh) => {
                            t.visibility.insert(done.key, mesh.visibility);
                            Update::Mesh(done.key, mesh)
                        }
                        None => {
                            t.visibility.remove(&done.key);
                            Update::Remove(done.key)
                        }
                    });
                }
                Output::Light(light) => out.push(Update::Light(done.key, light)),
            }
        }
        let free = (self.workers.threads * 2).saturating_sub(t.in_flight.len());
        for key in t.next_jobs(camera_block, free) {
            t.dirty.remove(&key);
            t.relight.remove(&key);
            t.in_flight.insert(key);
            self.workers.submit(Job { key, work: Work::Mesh { version: t.versions[&key] } });
        }
        let ready: Vec<_> = t.relight.iter().copied().filter(|k| !t.in_flight.contains(k)).collect();
        for key in ready {
            t.relight.remove(&key);
            t.in_flight.insert(key);
            self.workers.submit(Job { key, work: Work::Light });
        }
    }
}

impl Tracked {
    fn apply(&mut self, change: ChunkChange) {
        match change {
            ChunkChange::Column { x, z } => {
                self.columns.insert((x, z));
                for (dx, dz) in neighbourhood() {
                    for sy in self.sections.clone() {
                        self.mark((x + dx, sy, z + dz));
                    }
                }
            }
            ChunkChange::Section { x, section_y, z } => {
                self.columns.insert((x, z));
                for (dx, dz) in neighbourhood() {
                    for dy in -1..=1 {
                        self.mark((x + dx, section_y + dy, z + dz));
                    }
                }
            }
            ChunkChange::Block { x, y, z } => {
                let mut keys = [(0, 0, 0); 27];
                let mut n = 0;
                for (dx, dz) in neighbourhood() {
                    for dy in -1..=1 {
                        let key = ((x + dx) >> 4, (y + dy) >> 4, (z + dz) >> 4);
                        if !keys[..n].contains(&key) {
                            keys[n] = key;
                            n += 1;
                        }
                    }
                }
                keys[..n].iter().for_each(|&k| self.mark(k));
            }
        }
    }

    fn light_changed(&mut self, key: SectionKey) {
        if self.sections.contains(&key.1) && self.columns.contains(&(key.0, key.2)) && !self.dirty.contains(&key) {
            self.relight.insert(key);
        }
    }

    fn mark(&mut self, key: SectionKey) {
        if !self.sections.contains(&key.1) || !self.columns.contains(&(key.0, key.2)) {
            return;
        }
        self.versions.insert(key, self.next_version);
        self.next_version += 1;
        self.dirty.insert(key);
    }

    fn drop_unloaded(&mut self, out: &mut Vec<Update>) {
        let gone: Vec<_> = self.columns.iter().copied().filter(|&(x, z)| self.world.get(x, z).is_none()).collect();
        for (x, z) in gone {
            self.columns.remove(&(x, z));
            for sy in self.sections.clone() {
                let key = (x, sy, z);
                self.dirty.remove(&key);
                self.relight.remove(&key);
                self.versions.remove(&key);
                self.visibility.remove(&key);
                out.push(Update::Remove(key));
            }
        }
    }

    fn next_jobs(&self, camera_block: IVec3, count: usize) -> Vec<SectionKey> {
        if count == 0 || self.dirty.is_empty() {
            return Vec::new();
        }
        let cam = camera_block >> IVec3::splat(4);
        let score = |&(x, y, z): &SectionKey| {
            let d: IVec3 = IVec3::new(x, y, z) - cam;
            let missing = neighbourhood().filter(|&(dx, dz)| !self.columns.contains(&(x + dx, z + dz))).count() as i32;
            d.length_squared() + missing * MISSING_NEIGHBOUR_COST
        };
        let mut candidates: Vec<(i32, SectionKey)> =
            self.dirty.iter().filter(|k| !self.in_flight.contains(k)).map(|k| (score(k), *k)).collect();
        if candidates.len() > count {
            candidates.select_nth_unstable(count);
            candidates.truncate(count);
        }
        candidates.into_iter().map(|(_, k)| k).collect()
    }
}

fn neighbourhood() -> impl Iterator<Item = (i32, i32)> {
    (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (dx, dz)))
}
