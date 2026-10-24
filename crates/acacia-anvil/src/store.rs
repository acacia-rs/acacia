//! Converted columns waiting for their neighbours, lit and written region by region.
//!
//! A column is lit once all eight neighbours are in memory, or when it has waited `max_wait` (an edge of
//! the explored area). Columns lit without some neighbour are relit when one arrives. Written columns stay
//! as neighbours until the memory bound evicts the least recently converted ones.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use acacia_nbt::Value;
use acacia_world::{BlockRegistry, Chunk};

use crate::Error;
use crate::blockentity;
use crate::column::{Converter, JavaColumn, Scratch};
use crate::encode::{EncodeScratch, encode_chunk};
use crate::fixup::Fixups;
use crate::light::{Lighter, Neighbourhood};
use crate::mapping::JavaState;
use crate::props::PropsTable;
use crate::region::{Region, region_file_name};
use crate::section::VOLUME;

pub struct StoreConfig {
    /// The dimension's `region/` folder (e.g. `<world>/dimensions/minecraft/overworld/region`).
    pub region_dir: PathBuf,
    pub has_sky: bool,
    /// Longest a column waits for missing neighbours before it is lit without them.
    pub max_wait: Duration,
    /// A region file is rewritten at most this often (BlueMap's watcher re-renders a whole region).
    pub region_interval: Duration,
    /// Rough cap on converted columns held in memory.
    pub memory_limit: usize,
}

struct Entry {
    col: JavaColumn,
    converted: Instant,
    /// Hash of the chunk NBT last written, so an unchanged relight keeps the chunk's timestamp.
    written: Option<u64>,
}

fn hash_bytes(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FlushStats {
    pub columns: usize,
    pub regions: usize,
    pub waiting: usize,
}

pub struct AnvilStore {
    registry: Arc<BlockRegistry>,
    fixups: Fixups,
    buf: Box<[JavaState; VOLUME]>,
    cfg: StoreConfig,
    conv: Converter,
    props: PropsTable,
    columns: HashMap<(i32, i32), Entry>,
    /// Columns to (re)light and write, with when they first became dirty.
    dirty: HashMap<(i32, i32), Instant>,
    region_written: HashMap<(i32, i32), Instant>,
    bytes: usize,
    scratch: Scratch,
    lighter: Lighter,
    enc: EncodeScratch,
    nbt: Vec<u8>,
}

fn region_of((x, z): (i32, i32)) -> (i32, i32) {
    (x >> 5, z >> 5)
}

const ALL: u16 = 0x1FF;

impl AnvilStore {
    pub fn new(registry: Arc<BlockRegistry>, cfg: StoreConfig) -> Result<Self, Error> {
        std::fs::create_dir_all(&cfg.region_dir)
            .map_err(|e| Error::Io(cfg.region_dir.clone(), e))?;
        let conv = Converter::new(&registry);
        let props = PropsTable::new(&registry, &conv.map);
        let fixups = Fixups::new();
        Ok(Self {
            registry,
            fixups,
            buf: Box::new([0; VOLUME]),
            cfg,
            conv,
            props,
            columns: HashMap::new(),
            dirty: HashMap::new(),
            region_written: HashMap::new(),
            bytes: 0,
            scratch: Scratch::default(),
            lighter: Lighter::default(),
            enc: EncodeScratch::default(),
            nbt: Vec::new(),
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
                }
            }
        }
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

    fn write_region(&mut self, (rx, rz): (i32, i32), cols: &[(i32, i32)]) -> Result<(), Error> {
        let path = self.cfg.region_dir.join(region_file_name(rx, rz));
        let mut region = Region::load(&path)?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);
        let mut changed = false;
        for &pos in cols {
            self.dirty.remove(&pos);
            let mut edits = self
                .fixups
                .edits(&neighbourhood(&self.columns, pos), &self.props);
            if !edits.is_empty() {
                let entry = self.columns.get_mut(&pos).unwrap();
                self.bytes -= entry.col.heap_bytes();
                entry.col.apply_edits(&mut edits, &mut self.buf);
                self.bytes += entry.col.heap_bytes();
            }
            let n = neighbourhood(&self.columns, pos);
            let light = self.lighter.light(&n, &self.props, self.cfg.has_sky);
            encode_chunk(
                n.centre(),
                &light,
                &[],
                &self.props,
                &mut self.enc,
                &mut self.nbt,
            );
            let hash = hash_bytes(&self.nbt);
            let (lx, lz) = ((pos.0 & 31) as usize, (pos.1 & 31) as usize);
            let entry = self.columns.get_mut(&pos).unwrap();
            if entry.written == Some(hash) && region.has_chunk(lx, lz) {
                continue;
            }
            entry.written = Some(hash);
            // BlueMap re-renders a chunk only when its timestamp differs from the last render's.
            let ts = region
                .timestamp(lx, lz)
                .map_or(secs, |old| secs.max(old.wrapping_add(1)));
            region.set_chunk(lx, lz, &self.nbt, ts)?;
            changed = true;
        }
        if changed { region.save(&path) } else { Ok(()) }
    }
}

fn neighbourhood(columns: &HashMap<(i32, i32), Entry>, pos: (i32, i32)) -> Neighbourhood<'_> {
    let mut cols = [None; 9];
    for (i, slot) in cols.iter_mut().enumerate() {
        let p = (pos.0 + i as i32 % 3 - 1, pos.1 + i as i32 / 3 - 1);
        *slot = columns.get(&p).map(|e| &e.col);
    }
    Neighbourhood { cols }
}

impl AnvilStore {
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
