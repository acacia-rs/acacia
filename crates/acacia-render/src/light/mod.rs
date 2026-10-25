//! Block and sky light, computed on the client (Bedrock sends none). A light thread applies world
//! changes to [`LightData`], then forwards them with the sections whose light changed, so the
//! scene meshes only lit sections. See README "Lighting".

pub mod blocks;
pub mod column;
pub mod data;
pub mod environment;
pub mod lightmap;
mod propagate;
pub mod rules;
mod sections;
mod sky;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use acacia_world::{ChunkChange, World};
use crossbeam_channel::{Sender, unbounded};
use parking_lot::RwLock;
use rustc_hash::FxHashSet;

use blocks::PropsTable;
use column::WorldLighter;
pub use data::{LightData, LightVolume};

use crate::blocks::BlockTable;
use crate::workers::SectionKey;

/// How often the light thread looks for columns every view has dropped.
const UNLOAD_CHECK: Duration = Duration::from_millis(500);

pub enum LightEvent {
    /// A world change, applied to the light first.
    World(ChunkChange),
    /// The section's bordered light volume changed.
    Light(SectionKey),
}

pub struct Lighting {
    pub data: Arc<RwLock<LightData>>,
    pub events: crossbeam_channel::Receiver<LightEvent>,
}

impl Lighting {
    /// Starts the light thread, lighting by the rules of the look `table` draws; it lights the
    /// columns already loaded first. It stops once this handle is dropped.
    pub fn new(world: Arc<World>, table: &BlockTable) -> Lighting {
        let changes = world.subscribe();
        let data = Arc::new(RwLock::new(LightData::default()));
        let (tx, events) = unbounded();
        let shared = data.clone();
        let props = PropsTable::of_look(world.registry(), table);
        std::thread::Builder::new()
            .name("light".into())
            .spawn(move || run(&world, &props, &shared, &changes, &tx))
            .expect("spawn light thread");
        Lighting { data, events }
    }
}

fn run(world: &World, table: &PropsTable, data: &RwLock<LightData>, changes: &Receiver<ChunkChange>, tx: &Sender<LightEvent>) {
    let mut dim = world.dimension();
    // Java's End has sky light; Bedrock's has none.
    dim.sky |= table.rules().is_java() && crate::sky::Realm::of(&dim) == crate::sky::Realm::End;
    let lighter = WorldLighter::new(world, table, dim);
    let mut batch: Vec<ChunkChange> = world.chunk_positions().into_iter().map(|(x, z)| ChunkChange::Column { x, z }).collect();
    let mut last_unload_check = Instant::now();
    loop {
        if batch.is_empty() {
            match changes.recv_timeout(UNLOAD_CHECK) {
                Ok(c) => batch.push(c),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        batch.extend(changes.try_iter());
        if last_unload_check.elapsed() >= UNLOAD_CHECK {
            last_unload_check = Instant::now();
            let mut d = data.write();
            let gone: Vec<_> = d.columns.iter().copied().filter(|&(x, z)| world.get(x, z).is_none()).collect();
            gone.into_iter().for_each(|(x, z)| lighter.drop_column(&mut d, x, z));
        }
        apply(&lighter, data, &batch);
        let mut d = data.write();
        let touched = d.take_touched();
        d.generation += 1;
        drop(d);
        let events = batch.drain(..).map(LightEvent::World).chain(touched.into_iter().map(LightEvent::Light));
        for e in events {
            if tx.send(e).is_err() {
                return;
            }
        }
    }
}

/// Relights each column with a column or section change once, and updates single blocks elsewhere.
/// The lock is taken per column: a whole batch (a join's view) takes seconds, and each frame reads the
/// light to shade entities.
fn apply(lighter: &WorldLighter, data: &RwLock<LightData>, batch: &[ChunkChange]) {
    let mut relight: FxHashSet<(i32, i32)> = FxHashSet::default();
    for c in batch {
        if let ChunkChange::Column { x, z } | ChunkChange::Section { x, z, .. } = *c {
            relight.insert((x, z));
        }
    }
    for &(x, z) in &relight {
        lighter.relight_column(&mut data.write(), x, z);
    }
    let mut d = data.write();
    for c in batch {
        if let ChunkChange::Block { x, y, z } = *c
            && !relight.contains(&(x >> 4, z >> 4))
        {
            lighter.update_block(&mut d, x, y, z);
        }
    }
}
