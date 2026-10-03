//! Tracks which sections need meshing from world change events and feeds the worker pool, nearest
//! first. Knows nothing about the GPU: it yields [`Update`]s for the renderer to apply.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use acacia_world::{ChunkChange, World};
use glam::IVec3;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::blocks::BlockTable;
use crate::mesh::SectionMesh;
use crate::workers::{Job, SectionKey, Workers};

pub enum Update {
    Mesh(SectionKey, SectionMesh),
    Remove(SectionKey),
}

/// Frames between checks for columns every view has dropped.
const UNLOAD_CHECK_FRAMES: u64 = 30;
/// Sorting weight of each unloaded neighbour column: such a section will likely need remeshing
/// once the neighbour arrives, so it waits behind complete ones.
const MISSING_NEIGHBOUR_COST: i32 = 4096;

pub struct Scene {
    workers: Workers,
    tracked: Tracked,
    frame: u64,
}

struct Tracked {
    world: Arc<World>,
    changes: Receiver<ChunkChange>,
    columns: FxHashSet<(i32, i32)>,
    dirty: FxHashSet<SectionKey>,
    in_flight: FxHashSet<SectionKey>,
    /// Newest requested version per section; results of older jobs are dropped.
    versions: FxHashMap<SectionKey, u64>,
    next_version: u64,
    sections: std::ops::Range<i32>,
}

impl Scene {
    pub fn new(world: Arc<World>, table: Arc<BlockTable>) -> Self {
        let changes = world.subscribe();
        let dim = world.dimension();
        let min = dim.min_y >> 4;
        let mut t = Tracked {
            world: world.clone(),
            changes,
            columns: FxHashSet::default(),
            dirty: FxHashSet::default(),
            in_flight: FxHashSet::default(),
            versions: FxHashMap::default(),
            next_version: 1,
            sections: min..min + (dim.height / 16) as i32,
        };
        for (x, z) in world.chunk_positions() {
            t.apply(ChunkChange::Column { x, z });
        }
        Scene { workers: Workers::new(table), tracked: t, frame: 0 }
    }

    pub fn world(&self) -> &Arc<World> {
        &self.tracked.world
    }

    pub fn pending(&self) -> usize {
        self.tracked.dirty.len() + self.tracked.in_flight.len()
    }

    pub fn pump(&mut self, camera_block: IVec3, out: &mut Vec<Update>) {
        self.frame += 1;
        let t = &mut self.tracked;
        while let Ok(change) = t.changes.try_recv() {
            t.apply(change);
        }
        if self.frame.is_multiple_of(UNLOAD_CHECK_FRAMES) {
            t.drop_unloaded(out);
        }
        for done in self.workers.finished() {
            t.in_flight.remove(&done.key);
            if t.versions.get(&done.key) != Some(&done.version) {
                continue;
            }
            t.versions.remove(&done.key);
            out.push(match done.mesh {
                Some(mesh) => Update::Mesh(done.key, mesh),
                None => Update::Remove(done.key),
            });
        }
        let free = (self.workers.threads * 2).saturating_sub(t.in_flight.len());
        for key in t.next_jobs(camera_block, free) {
            t.dirty.remove(&key);
            t.in_flight.insert(key);
            self.workers.submit(Job { key, version: t.versions[&key], world: t.world.clone() });
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
                self.versions.remove(&key);
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
