//! Chunk compression options on a corpus of region files, one thread:
//! `cargo run --release -p acacia-anvil --example codec -- <dir>… [--iters N] [--write <compression> <out>]`.
//! Every `.mca` under each dir is read; per option it prints compress and decompress time per chunk, payload
//! bytes and on-disk bytes (whole 4 KiB sectors). `--write` rewrites the corpus to `<out>` (same relative
//! paths, timestamps kept) with `zlib[:level]|lz4|none`, for read and render comparisons.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use acacia_anvil::Compression;
use acacia_anvil::codec::{compress, decode_payload};
use acacia_anvil::region::Region;

struct Chunk {
    file: PathBuf,
    slot: usize,
    ts: u32,
    nbt: Vec<u8>,
}

fn mca_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            mca_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "mca") {
            out.push(p);
        }
    }
}

fn corpus(dirs: &[PathBuf]) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    for dir in dirs {
        let mut files = Vec::new();
        mca_files(dir, &mut files);
        files.sort();
        for f in files {
            let region = Region::load(&f).expect("region");
            let rel = f.strip_prefix(dir).unwrap().to_owned();
            for slot in 0..1024 {
                let (x, z) = (slot & 31, slot >> 5);
                if let Some(nbt) = region.chunk_nbt(x, z) {
                    let ts = region.timestamp(x, z).unwrap();
                    chunks.push(Chunk { file: rel.clone(), slot, ts, nbt });
                }
            }
        }
    }
    chunks
}

/// Fastest of `iters` passes, per chunk.
fn time(iters: usize, n: usize, mut pass: impl FnMut()) -> f64 {
    let best = (0..iters)
        .map(|_| {
            let t = Instant::now();
            pass();
            t.elapsed()
        })
        .min()
        .unwrap_or(Duration::ZERO);
    best.as_secs_f64() * 1e9 / n as f64
}

fn libdeflate_zlib(c: &mut libdeflater::Compressor, nbt: &[u8], out: &mut Vec<u8>) {
    out.resize(c.zlib_compress_bound(nbt.len()) + 1, 0);
    out[0] = 2;
    let n = c.zlib_compress(nbt, &mut out[1..]).unwrap();
    out.truncate(n + 1);
}

fn report(name: &str, chunks: &[Chunk], payloads: &[Vec<u8>], comp_ns: f64, iters: usize) {
    let n = chunks.len() as f64;
    let bytes: usize = payloads.iter().map(Vec::len).sum();
    let disk: usize = payloads.iter().map(|p| (p.len() + 4).div_ceil(4096) * 4096).sum();
    let mut d = libdeflater::Decompressor::new();
    let mut out = vec![0u8; 1 << 20];
    let dec_ns = time(iters, payloads.len(), || {
        for p in payloads {
            match p[0] {
                2 => {
                    d.zlib_decompress(&p[1..], &mut out).unwrap();
                }
                _ => {
                    std::hint::black_box(decode_payload(p).unwrap());
                }
            }
        }
    });
    println!(
        "{name:<14} compress {:>7.1} us  decompress {:>6.1} us  payload {:>7.0} B  disk {:>7.0} B  sectors {:.2}  88M chunks {:>6.0} GB",
        comp_ns / 1e3,
        dec_ns / 1e3,
        bytes as f64 / n,
        disk as f64 / n,
        disk as f64 / n / 4096.0,
        disk as f64 / n * 88e6 / 1e9
    );
}

fn write(chunks: &[Chunk], c: Compression, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::fs::remove_dir_all(out);
    let mut i = 0;
    while i < chunks.len() {
        let file = &chunks[i].file;
        let mut region = Region::default();
        while i < chunks.len() && chunks[i].file == *file {
            let ch = &chunks[i];
            region.set_chunk(ch.slot & 31, ch.slot >> 5, &ch.nbt, ch.ts, c)?;
            i += 1;
        }
        let path = out.join(file);
        std::fs::create_dir_all(path.parent().unwrap())?;
        region.save(&path)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut dirs, mut iters, mut target) = (Vec::new(), 5, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--iters" => iters = args.next().ok_or("--iters N")?.parse()?,
            "--write" => {
                let c: Compression = args.next().ok_or("--write <compression> <out>")?.parse()?;
                target = Some((c, PathBuf::from(args.next().ok_or("--write <compression> <out>")?)));
            }
            _ => dirs.push(PathBuf::from(a)),
        }
    }
    let chunks = corpus(&dirs);
    let raw: usize = chunks.iter().map(|c| c.nbt.len()).sum();
    println!("{} chunks, NBT {:.0} B each", chunks.len(), raw as f64 / chunks.len() as f64);
    if let Some((c, out)) = target {
        write(&chunks, c, &out)?;
        println!("wrote {} as {c:?}", out.display());
        return Ok(());
    }
    let n = chunks.len();
    let mut payloads = vec![Vec::new(); n];
    for (name, c) in [
        ("zlib-rs 6", Compression::Zlib(6)),
        ("zlib-rs 3", Compression::Zlib(3)),
        ("zlib-rs 1", Compression::Zlib(1)),
        ("lz4", Compression::Lz4),
        ("none", Compression::None),
    ] {
        let ns = time(iters, n, || {
            for (ch, p) in chunks.iter().zip(&mut payloads) {
                *p = compress(&ch.nbt, c).unwrap();
            }
        });
        report(name, &chunks, &payloads, ns, iters);
    }
    for level in [1, 3, 6, 9] {
        let mut comp = libdeflater::Compressor::new(libdeflater::CompressionLvl::new(level).unwrap());
        let ns = time(iters, n, || {
            for (ch, p) in chunks.iter().zip(&mut payloads) {
                libdeflate_zlib(&mut comp, &ch.nbt, p);
            }
        });
        report(&format!("libdeflate {level}"), &chunks, &payloads, ns, iters);
    }
    Ok(())
}
