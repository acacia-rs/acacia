//! Writing one region: neighbour fixups (sequential, they edit stored columns), then light, encode and
//! compress per column on the worker threads, then the region file.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{AnvilStore, Entry};
use crate::Error;
use crate::encode::{EncodeScratch, encode_chunk};
use crate::light::{Lighter, Neighbourhood};
use crate::props::PropsTable;
use crate::region::{self, Region, region_file_name};

/// Per-thread buffers.
#[derive(Default)]
pub(super) struct Worker {
    lighter: Lighter,
    enc: EncodeScratch,
    nbt: Vec<u8>,
}

/// A column's chunk payload, or `None` when its NBT is what was last written.
type Rendered = ((i32, i32), u64, Option<Vec<u8>>);

impl Worker {
    fn render(
        &mut self,
        columns: &HashMap<(i32, i32), Entry>,
        props: &PropsTable,
        has_sky: bool,
        region: &Region,
        pos: (i32, i32),
    ) -> Result<Rendered, Error> {
        let n = neighbourhood(columns, pos);
        let light = self.lighter.light(&n, props, has_sky);
        encode_chunk(n.centre(), &light, &[], props, &mut self.enc, &mut self.nbt);
        let hash = hash_bytes(&self.nbt);
        let unchanged =
            columns[&pos].written == Some(hash) && region.has_chunk(local(pos.0), local(pos.1));
        Ok((
            pos,
            hash,
            if unchanged {
                None
            } else {
                Some(region::compress(&self.nbt)?)
            },
        ))
    }
}

fn local(c: i32) -> usize {
    (c & 31) as usize
}

fn hash_bytes(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

pub(super) fn neighbourhood(
    columns: &HashMap<(i32, i32), Entry>,
    pos: (i32, i32),
) -> Neighbourhood<'_> {
    let mut cols = [None; 9];
    for (i, slot) in cols.iter_mut().enumerate() {
        let p = (pos.0 + i as i32 % 3 - 1, pos.1 + i as i32 / 3 - 1);
        *slot = columns.get(&p).map(|e| &e.col);
    }
    Neighbourhood { cols }
}

impl AnvilStore {
    pub(super) fn write_region(
        &mut self,
        (rx, rz): (i32, i32),
        cols: &[(i32, i32)],
    ) -> Result<(), Error> {
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

        let (columns, props, has_sky, region_ref) =
            (&self.columns, &self.props, self.cfg.has_sky, &region);
        let per = cols.len().div_ceil(self.workers.len()).max(4);
        let rendered: Vec<Result<Vec<Rendered>, Error>> = std::thread::scope(|s| {
            let jobs: Vec<_> = self
                .workers
                .iter_mut()
                .zip(cols.chunks(per))
                .map(|(w, batch)| {
                    s.spawn(move || {
                        batch
                            .iter()
                            .map(|&p| w.render(columns, props, has_sky, region_ref, p))
                            .collect()
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
        if changed { region.save(&path) } else { Ok(()) }
    }
}
