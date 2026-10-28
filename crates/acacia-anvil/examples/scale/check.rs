//! Correctness after a crawl: a sample region converted again in one go (everything in memory, its
//! neighbour ring included) must hold the same chunk NBT as the crawled one. Also the process's memory.
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use acacia_anvil::region::{Region, region_file_name};
use acacia_anvil::{AnvilStore, StoreConfig};
use acacia_world::BlockRegistry;

use crate::terrain::Terrain;

/// Chunks compared and how many differ, over `regions`.
pub fn regions(
    reg: &Arc<BlockRegistry>,
    terrain: &Terrain,
    revs: &HashMap<(i32, i32), u32>,
    side: i32,
    crawled: &Path,
    scratch: &Path,
    regions: &[(i32, i32)],
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let (mut compared, mut differ) = (0, 0);
    for &(rx, rz) in regions {
        let _ = std::fs::remove_dir_all(scratch);
        let cfg = StoreConfig {
            region_dir: scratch.to_owned(),
            has_sky: true,
            max_wait: Duration::ZERO,
            region_interval: Duration::ZERO,
            region_hold: Duration::ZERO,
            memory_limit: usize::MAX,
            threads: 0,
            compression: Default::default(),
        };
        let mut store = AnvilStore::new(reg.clone(), cfg)?;
        let lo = |r: i32| (r * 32 - 1).max(0);
        let hi = |r: i32| (r * 32 + 33).min(side);
        for cz in lo(rz)..hi(rz) {
            for cx in lo(rx)..hi(rx) {
                let rev = revs.get(&(cx, cz)).copied().unwrap_or(0);
                store.update(&terrain.chunk(cx, cz, rev), []);
            }
        }
        store.flush(true)?;
        let name = region_file_name(rx, rz);
        let (a, b) = (Region::load(&crawled.join(&name))?, Region::load(&scratch.join(&name))?);
        for i in 0..1024 {
            let (x, z) = (i & 31, i >> 5);
            let (na, nb) = (a.chunk_nbt(x, z), b.chunk_nbt(x, z));
            if na.is_none() && nb.is_none() {
                continue;
            }
            compared += 1;
            if na != nb {
                differ += 1;
                if differ <= 10 {
                    println!("  differs: chunk {} {}", rx * 32 + x as i32, rz * 32 + z as i32);
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(scratch);
    Ok((compared, differ))
}

/// Working set and its peak, in bytes.
#[cfg(windows)]
pub fn memory() -> (usize, usize) {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_working_set: usize,
        working_set: usize,
        rest: [usize; 6],
    }
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters {
        cb: size_of::<Counters>() as u32,
        ..Counters::default()
    };
    // SAFETY: a correctly sized PROCESS_MEMORY_COUNTERS for the current process.
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (c.working_set, c.peak_working_set)
}

#[cfg(not(windows))]
pub fn memory() -> (usize, usize) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |key: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<usize>().ok())
            .unwrap_or(0)
            * 1024
    };
    (kb("VmRSS:"), kb("VmHWM:"))
}
