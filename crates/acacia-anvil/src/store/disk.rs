//! Columns already written to region files, read back when one is needed again after eviction: as a
//! neighbour for light and fixups, or to be relit itself when a neighbour changes.

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use crate::Error;
use crate::column::JavaColumn;
use crate::decode::decode_chunk;
use crate::region::{Region, decode_payload, read_offsets, read_payload_at, region_file_name};

use super::region_of;

/// Regions whose presence bits are kept (~2 MB); 300k² blocks has ~341k regions.
const KNOWN: usize = 16384;

/// The region read last, kept open with its offset table; neighbours read back mostly share a region.
struct Open {
    region: (i32, i32),
    path: PathBuf,
    file: File,
    offsets: Box<[u32; 1024]>,
}

pub(super) struct Disk {
    dir: PathBuf,
    /// Per region file: one bit per chunk slot it holds; forgotten past `KNOWN` regions, rebuilt from the
    /// offset table on the next look.
    present: HashMap<(i32, i32), [u64; 16]>,
    open: Option<Open>,
}

fn slot((x, z): (i32, i32)) -> usize {
    ((x & 31) | ((z & 31) << 5)) as usize
}

/// The column at `pos` as last written, and its chunk NBT, from a [`Disk::payload`].
pub(super) fn decode(pos: (i32, i32), payload: &[u8]) -> Option<(JavaColumn, Vec<u8>)> {
    let nbt = decode_payload(payload).ok()?;
    let col = decode_chunk(&nbt).ok().filter(|c| (c.x, c.z) == pos)?;
    Some((col, nbt))
}

impl Disk {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            present: HashMap::new(),
            open: None,
        }
    }

    fn open(&mut self, region: (i32, i32)) -> Result<Option<&mut Open>, Error> {
        if self.open.as_ref().is_none_or(|o| o.region != region) {
            let path = self.dir.join(region_file_name(region.0, region.1));
            self.open = read_offsets(&path)?.map(|(file, offsets)| Open {
                region,
                path,
                file,
                offsets,
            });
        }
        Ok(self.open.as_mut())
    }

    pub fn has(&mut self, pos: (i32, i32)) -> Result<bool, Error> {
        let region = region_of(pos);
        if !self.present.contains_key(&region) {
            let mut bits = [0u64; 16];
            if let Some(open) = self.open(region)? {
                for (i, &loc) in open.offsets.iter().enumerate() {
                    if loc != 0 {
                        bits[i >> 6] |= 1 << (i & 63);
                    }
                }
            }
            self.remember(region, bits);
        }
        let i = slot(pos);
        Ok(self.present[&region][i >> 6] & (1 << (i & 63)) != 0)
    }

    /// The stored payload of the column, for [`decode`]; `None` if it is not on disk or unreadable.
    pub fn payload(&mut self, pos: (i32, i32)) -> Result<Option<Vec<u8>>, Error> {
        if !self.has(pos)? {
            return Ok(None);
        }
        let Some(open) = self.open(region_of(pos))? else {
            return Ok(None);
        };
        let loc = open.offsets[slot(pos)];
        read_payload_at(&mut open.file, &open.path, loc)
    }

    fn remember(&mut self, region: (i32, i32), bits: [u64; 16]) {
        if self.present.len() >= KNOWN && !self.present.contains_key(&region) {
            self.present.clear();
        }
        self.present.insert(region, bits);
    }

    pub fn regions_known(&self) -> usize {
        self.present.len()
    }

    /// Lets go of `region`'s file before it is replaced.
    pub fn close(&mut self, region: (i32, i32)) {
        if self.open.as_ref().is_some_and(|o| o.region == region) {
            self.open = None;
        }
    }

    /// Takes note of a region file just rewritten from `file`.
    pub fn saved(&mut self, region: (i32, i32), file: &Region) {
        let mut bits = [0u64; 16];
        for i in 0..1024 {
            if file.has_chunk(i & 31, i >> 5) {
                bits[i >> 6] |= 1 << (i & 63);
            }
        }
        self.remember(region, bits);
        self.close(region);
    }
}
