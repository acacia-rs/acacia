//! Anvil region files (`r.<rx>.<rz>.mca`): 32×32 chunks, a 4 KiB offset table, a 4 KiB timestamp table,
//! then each chunk as `[len u32 BE][compression u8][data]` padded to whole 4 KiB sectors.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Error;
pub use crate::codec::{Compression, compress, decode_payload};

const SECTOR: usize = 4096;

/// One chunk as stored: compression byte followed by the compressed NBT.
#[derive(Clone)]
struct Slot {
    payload: Vec<u8>,
    timestamp: u32,
}

/// A region held in memory while chunks are replaced, then written back whole.
pub struct Region {
    slots: Vec<Option<Slot>>,
}

impl Default for Region {
    fn default() -> Self {
        Self {
            slots: vec![None; 1024],
        }
    }
}

pub fn region_file_name(rx: i32, rz: i32) -> String {
    format!("r.{rx}.{rz}.mca")
}

fn slot_index(local_x: usize, local_z: usize) -> usize {
    (local_x & 31) | ((local_z & 31) << 5)
}

impl Region {
    /// Reads an existing region so untouched chunks survive a rewrite. A missing file is an empty region;
    /// unreadable entries (truncated, external `.mcc`) are dropped, since we own every file we write.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(Error::Io(path.to_owned(), e)),
        };
        let mut region = Self::default();
        if bytes.len() < 2 * SECTOR {
            return Ok(region);
        }
        for i in 0..1024 {
            let loc = u32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
            let (offset, count) = ((loc >> 8) as usize * SECTOR, (loc & 0xFF) as usize);
            if offset < 2 * SECTOR || count == 0 || offset + 5 > bytes.len() {
                continue;
            }
            let len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            if len == 0 || offset + 4 + len > bytes.len() || bytes[offset + 4] & 0x80 != 0 {
                continue;
            }
            let ts = u32::from_be_bytes(
                bytes[SECTOR + i * 4..SECTOR + i * 4 + 4]
                    .try_into()
                    .unwrap(),
            );
            region.slots[i] = Some(Slot {
                payload: bytes[offset + 4..offset + 4 + len].to_vec(),
                timestamp: ts,
            });
        }
        Ok(region)
    }

    pub fn has_chunk(&self, local_x: usize, local_z: usize) -> bool {
        self.slots[slot_index(local_x, local_z)].is_some()
    }

    /// The chunk's compression byte (2 zlib, 3 none, 4 LZ4).
    pub fn compression_type(&self, local_x: usize, local_z: usize) -> Option<u8> {
        self.slots[slot_index(local_x, local_z)]
            .as_ref()
            .and_then(|s| s.payload.first().copied())
    }

    pub fn timestamp(&self, local_x: usize, local_z: usize) -> Option<u32> {
        self.slots[slot_index(local_x, local_z)]
            .as_ref()
            .map(|s| s.timestamp)
    }

    /// Compresses `nbt` (an uncompressed chunk root compound) and stores it with `timestamp` (epoch
    /// seconds; BlueMap re-renders a chunk whose timestamp moved past its last render).
    pub fn set_chunk(
        &mut self,
        local_x: usize,
        local_z: usize,
        nbt: &[u8],
        timestamp: u32,
        compression: Compression,
    ) -> Result<(), Error> {
        let payload = compress(nbt, compression)?;
        self.set_payload(local_x, local_z, payload, timestamp);
        Ok(())
    }

    /// Stores a payload from [`compress`].
    pub fn set_payload(
        &mut self,
        local_x: usize,
        local_z: usize,
        payload: Vec<u8>,
        timestamp: u32,
    ) {
        self.slots[slot_index(local_x, local_z)] = Some(Slot { payload, timestamp });
    }

    pub fn remove_chunk(&mut self, local_x: usize, local_z: usize) {
        self.slots[slot_index(local_x, local_z)] = None;
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// Size of the file [`Self::encode`] makes.
    pub fn encoded_len(&self) -> usize {
        let body: usize = self
            .slots
            .iter()
            .flatten()
            .map(|s| (s.payload.len() + 4).div_ceil(SECTOR) * SECTOR)
            .sum();
        2 * SECTOR + body
    }

    /// The whole file, chunks packed in slot order.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.encoded_len()];
        let mut sector = 2;
        for (i, slot) in self.slots.iter().enumerate() {
            let Some(slot) = slot else { continue };
            let count = (slot.payload.len() + 4).div_ceil(SECTOR);
            let loc = ((sector as u32) << 8) | count as u32;
            out[i * 4..i * 4 + 4].copy_from_slice(&loc.to_be_bytes());
            out[SECTOR + i * 4..SECTOR + i * 4 + 4].copy_from_slice(&slot.timestamp.to_be_bytes());
            let at = sector * SECTOR;
            out[at..at + 4].copy_from_slice(&(slot.payload.len() as u32).to_be_bytes());
            out[at + 4..at + 4 + slot.payload.len()].copy_from_slice(&slot.payload);
            sector += count;
        }
        out
    }

    /// Writes `path` through a temporary sibling and a rename, so a reader never sees a half-written region.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let tmp = tmp_path(path);
        fs::write(&tmp, self.encode()).map_err(|e| Error::Io(tmp.clone(), e))?;
        rename_retrying(&tmp, path)
    }
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Windows refuses a rename onto a file another process holds open without share-delete; retry briefly.
fn rename_retrying(from: &Path, to: &Path) -> Result<(), Error> {
    let mut last = None;
    for attempt in 0..10 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(std::time::Duration::from_millis(20 << attempt.min(4)));
    }
    Err(Error::Io(to.to_owned(), last.unwrap()))
}

pub type Offsets = Box<[u32; 1024]>;

/// A region file, open, and its offset table (`(sector << 8) | count` per slot); `None` when the file is
/// missing.
pub fn read_offsets(path: &Path) -> Result<Option<(fs::File, Offsets)>, Error> {
    use std::io::Read;
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::Io(path.to_owned(), e)),
    };
    let mut raw = vec![0u8; SECTOR];
    match f.read_exact(&mut raw) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(Error::Io(path.to_owned(), e)),
    }
    let mut table = Box::new([0u32; 1024]);
    for (t, b) in table.iter_mut().zip(raw.as_chunks::<4>().0) {
        *t = u32::from_be_bytes(*b);
    }
    Ok(Some((f, table)))
}

/// One chunk's payload ([`decode_payload`] decompresses it) from `f` (the file at `path`, from
/// [`read_offsets`]), read at its offset-table entry without loading the rest of the file; `None` for an
/// empty or unreadable slot.
pub fn read_payload_at(f: &mut fs::File, path: &Path, loc: u32) -> Result<Option<Vec<u8>>, Error> {
    use std::io::{Read, Seek, SeekFrom};
    let (offset, count) = ((loc >> 8) as u64 * SECTOR as u64, (loc & 0xFF) as usize);
    if offset < 2 * SECTOR as u64 || count == 0 {
        return Ok(None);
    }
    let io_err = |e| Error::Io(path.to_owned(), e);
    f.seek(SeekFrom::Start(offset)).map_err(io_err)?;
    let mut len = [0u8; 4];
    let mut payload = Vec::new();
    let read = f.read_exact(&mut len).and_then(|()| {
        let n = u32::from_be_bytes(len) as usize;
        if n == 0 || n + 4 > count * SECTOR {
            return Ok(false);
        }
        payload.resize(n, 0);
        f.read_exact(&mut payload).map(|()| true)
    });
    match read {
        Ok(true) => Ok(Some(payload)),
        Ok(false) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(io_err(e)),
    }
}

impl Region {
    /// The stored chunk's decompressed NBT, if present.
    pub fn chunk_nbt(&self, local_x: usize, local_z: usize) -> Option<Vec<u8>> {
        self.slots[slot_index(local_x, local_z)]
            .as_ref()
            .and_then(|s| decode_payload(&s.payload).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_through_file() {
        let dir = std::env::temp_dir().join(format!("acacia-anvil-region-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(region_file_name(-1, 2));
        let mut r = Region::default();
        let big: Vec<u8> = (0..20_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        r.set_chunk(0, 0, b"hello", 7, Compression::default()).unwrap();
        r.set_chunk(31, 31, &big, 9, Compression::Lz4).unwrap();
        r.save(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len() % SECTOR, 0);
        let back = Region::load(&path).unwrap();
        assert_eq!(back.chunk_nbt(0, 0).unwrap(), b"hello");
        assert_eq!(back.chunk_nbt(31, 31).unwrap(), big);
        assert_eq!(back.timestamp(31, 31), Some(9));
        assert!(!back.has_chunk(1, 0));
        fs::remove_dir_all(&dir).unwrap();
    }
}
