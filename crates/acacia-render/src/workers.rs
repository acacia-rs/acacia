//! Mesh-building thread pool. Jobs read the world themselves, so the render thread never takes
//! chunk locks.

use std::sync::Arc;

use acacia_world::World;
use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::biome::BiomeColors;
use crate::blocks::BlockTable;
use crate::mesh::{SectionMesh, Volume, mesh_section};

/// Column x, section y (world y >> 4), column z.
pub type SectionKey = (i32, i32, i32);

pub struct Job {
    pub key: SectionKey,
    pub version: u64,
    pub world: Arc<World>,
}

pub struct Done {
    pub key: SectionKey,
    pub version: u64,
    /// `None` when the column was unloaded before the job ran.
    pub mesh: Option<SectionMesh>,
}

pub struct Workers {
    jobs: Sender<Job>,
    done: Receiver<Done>,
    pub threads: usize,
}

impl Workers {
    pub fn new(table: Arc<BlockTable>, biomes: Arc<BiomeColors>) -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        let threads = (cores.saturating_sub(2)).clamp(1, 6);
        let (jobs, job_rx) = unbounded::<Job>();
        let (done_tx, done) = unbounded();
        for i in 0..threads {
            let (rx, tx, table, biomes) = (job_rx.clone(), done_tx.clone(), table.clone(), biomes.clone());
            std::thread::Builder::new()
                .name(format!("mesh-{i}"))
                .spawn(move || {
                    for job in rx {
                        let mesh = build(&job, &table, &biomes);
                        if tx.send(Done { key: job.key, version: job.version, mesh }).is_err() {
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

fn build(job: &Job, table: &BlockTable, biomes: &BiomeColors) -> Option<SectionMesh> {
    let (cx, sy, cz) = job.key;
    let dim = job.world.dimension();
    let index = usize::try_from(sy - (dim.min_y >> 4)).ok()?;
    let column = job.world.get(cx, cz)?;
    if column.read().section_uniform(index) == Some(dim.air) {
        return Some(SectionMesh::default());
    }
    drop(column);
    Volume::gather(&job.world, cx, sy, cz).map(|v| mesh_section(&v, table, biomes))
}
