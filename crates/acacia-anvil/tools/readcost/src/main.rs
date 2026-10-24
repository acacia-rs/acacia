//! Per region folder: bluemap-rs's `Region::read_chunk_into` (sectors preloaded, so this is decompression)
//! and `Chunk::parse`, per chunk, fastest of `--iters` passes on one thread.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bm_world::chunk::PaletteCache;
use bm_world::region::{Region, list_regions};
use bm_world::{Biomes, BlockStates, Chunk, ChunkContext, DimensionType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut dirs, mut iters) = (Vec::new(), 5);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--iters" => iters = args.next().ok_or("--iters N")?.parse()?,
            _ => dirs.push(PathBuf::from(a)),
        }
    }
    let dimension = DimensionType::builtin("minecraft:overworld").ok_or("no overworld")?;
    let (states, biomes, palette) = (BlockStates::default(), Biomes::default(), PaletteCache::default());
    let ctx = ChunkContext { states: &states, biomes: &biomes, dimension: &dimension, palette: Some(&palette) };
    for dir in dirs {
        let mut regions = Vec::new();
        for (x, z) in list_regions(&dir)? {
            let mut r = Region::open(&dir, x, z)?;
            r.preload((0..1024).map(|i| (i & 31, i >> 5)));
            regions.push(r);
        }
        let (mut raw, mut nbt) = (Vec::new(), Vec::new());
        let (mut best_read, mut best_parse, mut chunks, mut nbt_bytes) = (Duration::MAX, Duration::MAX, 0, 0);
        for _ in 0..iters {
            let (mut read, mut parse) = (Duration::ZERO, Duration::ZERO);
            (chunks, nbt_bytes) = (0, 0);
            for r in &regions {
                for i in 0..1024 {
                    let t = Instant::now();
                    if !r.read_chunk_into(i & 31, i >> 5, &mut raw, &mut nbt)? {
                        continue;
                    }
                    let t2 = Instant::now();
                    std::hint::black_box(Chunk::parse(&nbt, &ctx)?);
                    parse += t2.elapsed();
                    read += t2 - t;
                    chunks += 1;
                    nbt_bytes += nbt.len();
                }
            }
            best_read = best_read.min(read);
            best_parse = best_parse.min(parse);
        }
        let us = |d: Duration| d.as_secs_f64() * 1e6 / chunks as f64;
        println!(
            "{}: {chunks} chunks ({:.0} B NBT each), read+decompress {:.1} us, parse {:.1} us per chunk",
            dir.display(),
            nbt_bytes as f64 / chunks as f64,
            us(best_read),
            us(best_parse)
        );
    }
    Ok(())
}
