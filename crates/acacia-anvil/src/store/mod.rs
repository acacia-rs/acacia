//! Converted columns waiting for their neighbours, lit and written region by region.
//!
//! A column is lit once all eight neighbours are in memory, or when it has waited `max_wait` (an edge of
//! the explored area). Columns lit without some neighbour are relit when one arrives. Written columns stay
//! as neighbours until the memory bound evicts the least recently converted ones; an evicted column is
//! read back from its region file when it is needed as a neighbour or must be relit (`disk.rs`).

mod disk;
mod write;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_nbt::Value;
use acacia_world::{BlockRegistry, Chunk, Dimension};

use crate::Error;
use crate::blockentity;
use crate::column::{Converter, JavaColumn, Scratch};
use crate::fixup::Fixups;
use crate::mapping::JavaState;
use crate::props::PropsTable;
use crate::section::VOLUME;
use disk::Disk;
use write::{Worker, hash_bytes};

pub struct StoreConfig {
    /// The dimension's `region/` folder ([`region_dir`]).
    pub region_dir: PathBuf,
    pub has_sky: bool,
    /// Longest a column waits for missing neighbours before it is lit without them.
    pub max_wait: Duration,
    /// A region file is rewritten at most this often (BlueMap's watcher re-renders a whole region).
    pub region_interval: Duration,
    /// Rough cap on converted columns held in memory.
    pub memory_limit: usize,
    /// Threads lighting and encoding columns; 0 = one per core.
    pub threads: usize,
}

struct Entry {
    col: JavaColumn,
    converted: Instant,
    /// Hash of the chunk NBT last written, so an unchanged relight keeps the chunk's timestamp.
    written: Option<u64>,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FlushStats {
    pub columns: usize,
    pub regions: usize,
    pub waiting: usize,
}

pub struct AnvilStore {
    registry: Arc<BlockRegistry>,
    cfg: StoreConfig,
    conv: Converter,
    props: PropsTable,
    fixups: Fixups,
    columns: HashMap<(i32, i32), Entry>,
    /// Columns to (re)light and write, with when they first became dirty.
    dirty: HashMap<(i32, i32), Instant>,
    /// Neighbours of updated columns that were not in memory: relit if they are on disk.
    stale: HashMap<(i32, i32), Instant>,
    disk: Disk,
    region_written: HashMap<(i32, i32), Instant>,
    bytes: usize,
    scratch: Scratch,
    buf: Box<[JavaState; VOLUME]>,
    workers: Vec<Worker>,
}

/// `<world>/dimensions/minecraft/<dimension>/region`, the 26.x layout BlueMap looks in first. Bedrock
/// dimensions differ only in height here, which is what tells them apart.
pub fn region_dir(world: &Path, dim: Dimension) -> PathBuf {
    let name = match (dim.min_y, dim.height) {
        (0, 128) => "the_nether",
        (0, 256) => "the_end",
        _ => "overworld",
    };
    world.join("dimensions/minecraft").join(name).join("region")
}

fn region_of((x, z): (i32, i32)) -> (i32, i32) {
    (x >> 5, z >> 5)
}

const ALL: u16 = 0x1FF;

impl AnvilStore {
    pub fn new(registry: Arc<BlockRegistry>, cfg: StoreConfig) -> Result<Self, Error> {
        std::fs::create_dir_all(&cfg.region_dir)
            .map_err(|e| Error::Io(cfg.region_dir.clone(), e))?;
        let threads = match cfg.threads {
            0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
            n => n,
        };
        Ok(Self {
            conv: Converter::new(&registry),
            registry,
            disk: Disk::new(cfg.region_dir.clone()),
            cfg,
            props: PropsTable::new(),
            fixups: Fixups::new(),
            columns: HashMap::new(),
            dirty: HashMap::new(),
            stale: HashMap::new(),
            region_written: HashMap::new(),
            bytes: 0,
            scratch: Scratch::default(),
            buf: Box::new([0; VOLUME]),
            workers: (0..threads).map(|_| Worker::default()).collect(),
        })
    }

    pub fn converter(&self) -> &Converter {
        &self.conv
    }

    /// Converts a chunk (new or changed) with its block entities (world position, Bedrock NBT) and queues
    /// it and its neighbours for writing.
    pub fn update<'a>(
        &mut self,
        chunk: &Chunk,
        block_entities: impl IntoIterator<Item = ([i32; 3], &'a Value)>,
    ) {
        let mut col = self.conv.convert(chunk, &mut self.scratch);
        blockentity::apply(
            &mut col,
            block_entities,
            &self.registry,
            &self.conv.map,
            &mut self.buf,
        );
        let pos = (col.x, col.z);
        let now = Instant::now();
        self.bytes += col.heap_bytes();
        let mut entry = Entry {
            col,
            converted: now,
            written: None,
        };
        if let Some(old) = self.columns.remove(&pos) {
            self.bytes -= old.col.heap_bytes();
            entry.written = old.written;
        }
        self.columns.insert(pos, entry);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let p = (pos.0 + dx, pos.1 + dz);
                if self.columns.contains_key(&p) {
                    self.dirty.entry(p).or_insert(now);
                } else {
                    self.stale.entry(p).or_insert(now);
                }
            }
        }
    }

    /// Brings back from disk the stale columns (to be relit) and every neighbour a dirty column lacks.
    fn reload(&mut self) -> Result<(), Error> {
        for (p, since) in std::mem::take(&mut self.stale) {
            if self.load(p)? {
                self.dirty.entry(p).or_insert(since);
            }
        }
        let mut missing: Vec<(i32, i32)> = self
            .dirty
            .keys()
            .flat_map(|&(x, z)| {
                (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| (x + dx, z + dz)))
            })
            .filter(|p| !self.columns.contains_key(p))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        for p in missing {
            self.load(p)?;
        }
        Ok(())
    }

    /// Whether `pos` is in memory, reading it back from its region file if needed.
    fn load(&mut self, pos: (i32, i32)) -> Result<bool, Error> {
        if self.columns.contains_key(&pos) {
            return Ok(true);
        }
        let Some((col, nbt)) = self.disk.load(pos)? else {
            return Ok(false);
        };
        self.bytes += col.heap_bytes();
        let entry = Entry {
            col,
            converted: Instant::now(),
            written: Some(hash_bytes(&nbt)),
        };
        self.columns.insert(pos, entry);
        Ok(true)
    }

    fn present(&self, (x, z): (i32, i32)) -> u16 {
        let mut mask = 0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                if self.columns.contains_key(&(x + dx, z + dz)) {
                    mask |= 1 << ((dz + 1) * 3 + dx + 1);
                }
            }
        }
        mask
    }

    /// Lights and writes ready columns; `force` writes everything dirty now (shutdown).
    pub fn flush(&mut self, force: bool) -> Result<FlushStats, Error> {
        self.reload()?;
        let now = Instant::now();
        let mut by_region: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
        for (&pos, &since) in &self.dirty {
            let ready =
                force || now.duration_since(since) >= self.cfg.max_wait || self.present(pos) == ALL;
            let region = region_of(pos);
            let due = force
                || self
                    .region_written
                    .get(&region)
                    .is_none_or(|t| now.duration_since(*t) >= self.cfg.region_interval);
            if ready && due {
                by_region.entry(region).or_default().push(pos);
            }
        }
        let mut stats = FlushStats::default();
        for (region, cols) in by_region {
            self.write_region(region, &cols)?;
            self.region_written.insert(region, now);
            stats.columns += cols.len();
            stats.regions += 1;
        }
        stats.waiting = self.dirty.len();
        self.evict();
        Ok(stats)
    }

    /// Drops the oldest written columns above the memory bound; dirty ones and their neighbours stay.
    fn evict(&mut self) {
        if self.bytes <= self.cfg.memory_limit {
            return;
        }
        let keep: HashSet<(i32, i32)> = self
            .dirty
            .keys()
            .flat_map(|&(x, z)| {
                (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| (x + dx, z + dz)))
            })
            .collect();
        let mut old: Vec<((i32, i32), Instant)> = self
            .columns
            .iter()
            .filter(|(p, _)| !keep.contains(p))
            .map(|(p, e)| (*p, e.converted))
            .collect();
        old.sort_by_key(|&(_, t)| t);
        for (pos, _) in old {
            if self.bytes <= self.cfg.memory_limit * 9 / 10 {
                break;
            }
            if let Some(e) = self.columns.remove(&pos) {
                self.bytes -= e.col.heap_bytes();
            }
        }
    }

    pub fn columns(&self) -> usize {
        self.columns.len()
    }

    pub fn heap_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests;
