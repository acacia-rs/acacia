//! Runs one operation over the corpus in a loop, for a sampling profiler, after reporting what it
//! allocates (README.md, "Profiling").
//! `cargo run -p acacia-nbt --profile profiling --example profile -- <read|skip|raw|write> <network|le> [seconds]`

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

use acacia_nbt::{Flavor, LittleEndian, Nbt, Network, Raw, read, skip, write};
use bytes::BytesMut;

#[path = "../tests/common/corpus.rs"]
mod corpus;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED: AtomicU64 = AtomicU64::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Relaxed);
        ALLOCATED.fetch_add(layout.size() as u64, Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Relaxed);
        ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()) as u64, Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn run<F: Flavor>(op: &str, docs: &[Vec<u8>], seconds: f64) -> Result<(), String> {
    let trees: Vec<Nbt> = docs.iter().map(|d| read::<F>(&mut &d[..]).unwrap()).collect();
    let bytes: usize = docs.iter().map(Vec::len).sum();
    let mut out = BytesMut::with_capacity(bytes);
    let mut pass = || match op {
        "read" => docs.iter().for_each(|d| drop(black_box(read::<F>(&mut &d[..]).unwrap()))),
        "skip" => docs.iter().for_each(|d| skip::<F>(&mut black_box(&d[..])).unwrap()),
        "raw" => docs.iter().for_each(|d| drop(black_box(Raw::<F>::read(&mut &d[..]).unwrap()))),
        _ => {
            out.clear();
            trees.iter().for_each(|t| write::<F>(&mut out, t));
        }
    };

    let before = (ALLOCATIONS.load(Relaxed), ALLOCATED.load(Relaxed));
    pass();
    let allocations = ALLOCATIONS.load(Relaxed) - before.0;
    let allocated = ALLOCATED.load(Relaxed) - before.1;

    let (start, mut passes) = (Instant::now(), 0u64);
    while start.elapsed() < Duration::from_secs_f64(seconds) {
        pass();
        passes += 1;
    }
    let mb_per_s = (passes * bytes as u64) as f64 / start.elapsed().as_secs_f64() / 1e6;
    println!(
        "{op}: {} documents, {bytes} bytes; {mb_per_s:.0} MB/s; {:.1} allocations per document, {:.2} bytes allocated per input byte",
        docs.len(),
        allocations as f64 / docs.len() as f64,
        allocated as f64 / bytes as f64,
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(op @ ("read" | "skip" | "raw" | "write")), Some(flavour)) = (args.first().map(String::as_str), args.get(1)) else {
        return Err("usage: profile <read|skip|raw|write> <network|le> [seconds, default 5]".into());
    };
    let seconds = args.get(2).map_or(Ok(5.0), |s| s.parse::<f64>()).map_err(|e| e.to_string())?;
    let docs: Vec<Vec<u8>> = corpus::load(flavour).into_iter().map(|(_, bytes)| bytes).collect();
    match flavour.as_str() {
        "network" => run::<Network>(op, &docs, seconds),
        "le" => run::<LittleEndian>(op, &docs, seconds),
        other => Err(format!("unknown flavour {other}")),
    }
}
