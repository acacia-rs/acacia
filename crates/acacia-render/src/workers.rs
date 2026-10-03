//! Mesh-building thread pool. Jobs read the world and the light themselves, so the render thread
//! never takes chunk locks.

use std::sync::Arc;

use acacia_world::World;
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::RwLock;

use crate::biome::BiomeColors;
use crate::blocks::BlockTable;
use crate::light::{LightData, LightVolume};
use crate::mesh::{SectionMesh, Volume, mesh_section};

/// Column x, section y (world y >> 4), column z.
pub type SectionKey = (i32, i32, i32);

#[derive(Debug, Clone, Copy)]
pub enum Work {
    /// Mesh plus light; results of versions older than the newest requested are dropped.
    Mesh { version: u64 },
    /// Light only, for a section whose blocks didn't change.
    Light,
}

pub struct Job {
    pub key: SectionKey,
    pub work: Work,
}

pub enum Output {
    /// `None` when the column was unloaded before the job ran.
    Mesh { version: u64, mesh: Option<SectionMesh> },
    Light(LightVolume),
}

pub struct Done {
    pub key: SectionKey,
    pub output: Output,
}

pub struct Workers {
    jobs: Sender<Job>,
    done: Receiver<Done>,
    pub threads: usize,
}

struct Shared {
    world: Arc<World>,
    light: Arc<RwLock<LightData>>,
    table: Arc<BlockTable>,
    biomes: Arc<BiomeColors>,
}

impl Workers {
    pub fn new(world: Arc<World>, light: Arc<RwLock<LightData>>, table: Arc<BlockTable>, biomes: Arc<BiomeColors>) -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        let threads = (cores.saturating_sub(2)).clamp(1, 6);
        let (jobs, job_rx) = unbounded::<Job>();
        let (done_tx, done) = unbounded();
        let shared = Arc::new(Shared { world, light, table, biomes });
        for i in 0..threads {
            let (rx, tx, shared) = (job_rx.clone(), done_tx.clone(), shared.clone());
            std::thread::Builder::new()
                .name(format!("mesh-{i}"))
                .spawn(move || {
                    for job in rx {
                        let output = match job.work {
                            Work::Mesh { version } => Output::Mesh { version, mesh: build(job.key, &shared) },
                            Work::Light => Output::Light(shared.light.read().gather(job.key)),
                        };
                        if tx.send(Done { key: job.key, output }).is_err() {
                            break;
                        }
                    }
                })
                .expect("spawn mesh thread");
        }
        Workers { jobs, done, threads }
    }

    pub fn submit(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    pub fn finished(&self) -> impl Iterator<Item = Done> + '_ {
        self.done.try_iter()
    }
}

fn build((cx, sy, cz): SectionKey, s: &Shared) -> Option<SectionMesh> {
    let dim = s.world.dimension();
    let index = usize::try_from(sy - (dim.min_y >> 4)).ok()?;
    let column = s.world.get(cx, cz)?;
    if column.read().section_uniform(index) == Some(dim.air) {
        return Some(SectionMesh::default());
    }
    drop(column);
    let volume = Volume::gather(&s.world, cx, sy, cz)?;
    let mut mesh = mesh_section(&volume, &s.table, &s.biomes);
    if !mesh.is_empty() {
        mesh.light = Some(s.light.read().gather((cx, sy, cz)));
    }
    Some(mesh)
}
