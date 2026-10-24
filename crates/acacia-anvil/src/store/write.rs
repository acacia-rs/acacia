//! Writing one region: neighbour fixups (sequential, they edit stored columns), then on the worker threads
//! light (nearby dirty columns together over one volume, light.rs), encode and compress, then the file.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{AnvilStore, Entry};
use crate::Error;
use crate::column::JavaColumn;
use crate::encode::{EncodeScratch, encode_chunk};
use crate::light::{Lighter, Neighbourhood, SectionLight};
use crate::props::PropsTable;
use crate::region::{self, Region, region_file_name};

type Pos = (i32, i32);

/// Per-thread buffers.
#[derive(Default)]
pub(super) struct Worker {
    lighter: Lighter,
    enc: EncodeScratch,
    nbt: Vec<u8>,
}

/// A column's chunk payload, or `None` when its NBT is what was last written.
type Rendered = (Pos, u64, Option<Vec<u8>>);

/// What every worker reads.
struct Shared<'a> {
    columns: &'a HashMap<Pos, Entry>,
    props: &'a PropsTable,
    has_sky: bool,
    region: &'a Region,
}

impl Worker {
    /// Lights `group` over one volume padded by a column on each side, then encodes each column.
    fn render_group(
        &mut self,
        sh: &Shared,
        group: &[Pos],
        out: &mut Vec<Rendered>,
    ) -> Result<(), Error> {
        let ((x0, z0), (x1, z1)) = bbox(group);
        let w = (x1 - x0 + 3) as usize;
        let area: Vec<Option<&JavaColumn>> = (z0 - 1..=z1 + 1)
            .flat_map(|z| (x0 - 1..=x1 + 1).map(move |x| (x, z)))
            .map(|p| sh.columns.get(&p).map(|e| &e.col))
            .collect();
        self.lighter.light_area(&area, w, sh.props, sh.has_sky);
        for &pos in group {
            let col = &sh.columns[&pos].col;
            let (cx, cz) = ((pos.0 - x0 + 1) as usize, (pos.1 - z0 + 1) as usize);
            let light = self.lighter.extract(cx, cz, col.sections.len());
            out.push(self.encode(sh, pos, &light)?);
        }
        Ok(())
    }

    fn encode(&mut self, sh: &Shared, pos: Pos, light: &[SectionLight]) -> Result<Rendered, Error> {
        let entry = &sh.columns[&pos];
        encode_chunk(&entry.col, light, &[], sh.props, &mut self.enc, &mut self.nbt);
        let hash = hash_bytes(&self.nbt);
        let (lx, lz) = (local(pos.0), local(pos.1));
        // Not written by this store yet (evicted before, or a previous run): compare with the file.
        let unchanged = sh.region.has_chunk(lx, lz)
            && match entry.written {
                Some(h) => h == hash,
                None => sh.region.chunk_nbt(lx, lz).is_some_and(|old| old == self.nbt),
            };
        let payload = if unchanged {
            None
        } else {
            Some(region::compress(&self.nbt)?)
        };
        Ok((pos, hash, payload))
    }
}

fn bbox(group: &[Pos]) -> (Pos, Pos) {
    let (mut lo, mut hi) = (group[0], group[0]);
    for &(x, z) in group {
        lo = (lo.0.min(x), lo.1.min(z));
        hi = (hi.0.max(x), hi.1.max(z));
    }
    (lo, hi)
}

/// The dirty columns of each aligned square of `1 << shift` columns, largest first. A sparse square
/// costs more padded than lit column by column (3×3 each), so its columns go alone.
fn groups(cols: &[Pos], shift: u32) -> Vec<Vec<Pos>> {
    let mut by: HashMap<Pos, Vec<Pos>> = HashMap::new();
    for &p in cols {
        by.entry((p.0 >> shift, p.1 >> shift)).or_default().push(p);
    }
    let mut out = Vec::new();
    for g in by.into_values() {
        let ((x0, z0), (x1, z1)) = bbox(&g);
        if ((x1 - x0 + 3) * (z1 - z0 + 3)) as usize > 9 * g.len() {
            out.extend(g.into_iter().map(|p| vec![p]));
        } else {
            out.push(g);
        }
    }
    out.sort_by_key(|g| std::cmp::Reverse(g.len()));
    out
}

/// Groups as large as still give every worker one (8×8 columns, else 4×4, else 2×2).
fn grouping(cols: &[Pos], workers: usize) -> Vec<Vec<Pos>> {
    for shift in [3, 2] {
        let g = groups(cols, shift);
        if g.len() >= workers {
            return g;
        }
    }
    groups(cols, 1)
}

fn local(c: i32) -> usize {
    (c & 31) as usize
}

pub(super) fn hash_bytes(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

pub(super) fn neighbourhood(columns: &HashMap<Pos, Entry>, pos: Pos) -> Neighbourhood<'_> {
    let mut cols = [None; 9];
    for (i, slot) in cols.iter_mut().enumerate() {
        let p = (pos.0 + i as i32 % 3 - 1, pos.1 + i as i32 / 3 - 1);
        *slot = columns.get(&p).map(|e| &e.col);
    }
    Neighbourhood { cols }
}

impl AnvilStore {
    pub(super) fn write_region(&mut self, (rx, rz): Pos, cols: &[Pos]) -> Result<(), Error> {
        let path = self.cfg.region_dir.join(region_file_name(rx, rz));
        let mut region = Region::load(&path)?;
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
        }

        let groups = grouping(cols, self.workers.len());
        let sh = Shared {
            columns: &self.columns,
            props: &self.props,
            has_sky: self.cfg.has_sky,
            region: &region,
        };
        let next = AtomicUsize::new(0);
        let rendered: Vec<Result<Vec<Rendered>, Error>> = std::thread::scope(|s| {
            let jobs: Vec<_> = self
                .workers
                .iter_mut()
                .take(groups.len())
                .map(|w| {
                    let (sh, groups, next) = (&sh, &groups, &next);
                    s.spawn(move || {
                        let mut out = Vec::new();
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(g) = groups.get(i) else { break };
                            w.render_group(sh, g, &mut out)?;
                        }
                        Ok(out)
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|j| j.join().expect("render worker panicked"))
                .collect()
        });

        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);
        let mut changed = false;
        for (pos, hash, payload) in rendered
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
        {
            let Some(payload) = payload else { continue };
            let (lx, lz) = (local(pos.0), local(pos.1));
            // BlueMap re-renders a chunk only when its timestamp differs from the last render's.
            let ts = region
                .timestamp(lx, lz)
                .map_or(secs, |old| secs.max(old.wrapping_add(1)));
            region.set_payload(lx, lz, payload, ts);
            self.columns.get_mut(&pos).unwrap().written = Some(hash);
            changed = true;
        }
        if changed {
            region.save(&path)?;
            self.disk.saved((rx, rz), &region);
        }
        Ok(())
    }
}
