//! Columns already written to region files, read back when one is needed again after eviction: as a
//! neighbour for light and fixups, or to be relit itself when a neighbour changes.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::Error;
use crate::column::JavaColumn;
use crate::decode::decode_chunk;
use crate::region::{Region, read_chunk_at, read_offsets, region_file_name};

use super::region_of;

type Offsets = Box<[u32; 1024]>;

pub(super) struct Disk {
    dir: PathBuf,
    /// Per region file: one bit per chunk slot it holds (128 bytes a region, so 300k² blocks is ~45 MB).
    present: HashMap<(i32, i32), [u64; 16]>,
    /// The offset table of the region read last; neighbours read back mostly share a region.
    header: Option<((i32, i32), Offsets)>,
}

fn slot((x, z): (i32, i32)) -> usize {
    ((x & 31) | ((z & 31) << 5)) as usize
}

impl Disk {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            present: HashMap::new(),
            header: None,
        }
    }

    fn offsets(&mut self, region: (i32, i32)) -> Result<Option<&[u32; 1024]>, Error> {
        if self.header.as_ref().is_none_or(|(r, _)| *r != region) {
            let path = self.dir.join(region_file_name(region.0, region.1));
            self.header = read_offsets(&path)?.map(|t| (region, t));
            if self.header.is_none() {
                return Ok(None);
            }
        }
        Ok(self.header.as_ref().map(|(_, t)| &**t))
    }

    pub fn has(&mut self, pos: (i32, i32)) -> Result<bool, Error> {
        let region = region_of(pos);
        if !self.present.contains_key(&region) {
            let mut bits = [0u64; 16];
            if let Some(table) = self.offsets(region)? {
                for (i, &loc) in table.iter().enumerate() {
                    if loc != 0 {
                        bits[i >> 6] |= 1 << (i & 63);
                    }
                }
            }
            self.present.insert(region, bits);
        }
        let i = slot(pos);
        Ok(self.present[&region][i >> 6] & (1 << (i & 63)) != 0)
    }

    /// The column as last written and its chunk NBT; `None` if it is not on disk or unreadable.
    pub fn load(&mut self, pos: (i32, i32)) -> Result<Option<(JavaColumn, Vec<u8>)>, Error> {
        if !self.has(pos)? {
            return Ok(None);
        }
        let region = region_of(pos);
        let Some(&loc) = self.offsets(region)?.map(|t| &t[slot(pos)]) else {
            return Ok(None);
        };
        let path = self.dir.join(region_file_name(region.0, region.1));
        let Some(nbt) = read_chunk_at(&path, loc)? else {
            return Ok(None);
        };
        Ok(decode_chunk(&nbt)
            .ok()
            .filter(|c| (c.x, c.z) == pos)
            .map(|c| (c, nbt)))
    }

    /// Takes note of a region file just rewritten from `file`.
    pub fn saved(&mut self, region: (i32, i32), file: &Region) {
        let mut bits = [0u64; 16];
        for i in 0..1024 {
            if file.has_chunk(i & 31, i >> 5) {
                bits[i >> 6] |= 1 << (i & 63);
            }
        }
        self.present.insert(region, bits);
        if self.header.as_ref().is_some_and(|(r, _)| *r == region) {
            self.header = None;
        }
    }
}
