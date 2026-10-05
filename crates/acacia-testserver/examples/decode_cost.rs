//! What decoding the recorded BDS join costs, per packet type: time and allocations (docs/proto.md, "Decode cost").
//! `cargo run --release -p acacia-testserver --example decode_cost` prints the table;
//! `... -- <PacketName> [seconds]` decodes only that type in a loop, for a sampling profiler;
//! `--features mimalloc` measures with mimalloc in place of the system allocator.

use std::alloc::{GlobalAlloc, Layout};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

use acacia_proto::packets::ItemRegistry;
use acacia_proto::{RawPacket, manual, packet_name, strict};
use acacia_testserver::{Script, Step};

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED: AtomicU64 = AtomicU64::new(0);

/// Off in the profiler loop: the two atomic adds were 13% of a `CraftingData` profile.
static COUNTING: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "mimalloc")]
static INNER: mimalloc::MiMalloc = mimalloc::MiMalloc;
#[cfg(not(feature = "mimalloc"))]
static INNER: std::alloc::System = std::alloc::System;

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCATIONS.fetch_add(1, Relaxed);
            ALLOCATED.fetch_add(layout.size() as u64, Relaxed);
        }
        unsafe { INNER.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { INNER.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCATIONS.fetch_add(1, Relaxed);
            ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()) as u64, Relaxed);
        }
        unsafe { INNER.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const ROUNDS: u64 = 20;

#[derive(Default)]
struct Cost {
    packets: u64,
    bytes: u64,
    nanos: u64,
    allocations: u64,
    allocated: u64,
}

fn join() -> Vec<RawPacket> {
    let raws: Vec<RawPacket> = Script::bds_spawn()
        .steps
        .into_iter()
        .filter_map(|s| match s {
            Step::Send { packet, .. } => RawPacket::parse(packet).ok(),
            Step::Wait(_) => None,
        })
        .collect();
    // As the session does: item decoding branches on the server's shield id.
    for raw in raws.iter().filter(|r| r.is::<ItemRegistry>()) {
        if let Ok(Some(shield)) = manual::shield_item_id_in_registry(&raw.body) {
            manual::set_shield_item_id(shield);
        }
    }
    raws
}

fn table(raws: &[RawPacket]) {
    let mut costs: BTreeMap<&str, Cost> = BTreeMap::new();
    for _ in 0..ROUNDS {
        for raw in raws {
            let (allocations, allocated) = (ALLOCATIONS.load(Relaxed), ALLOCATED.load(Relaxed));
            let start = Instant::now();
            let _ = black_box(strict::check(raw));
            let nanos = start.elapsed().as_nanos() as u64;
            let c = costs.entry(packet_name(raw.id).unwrap_or("?")).or_default();
            c.nanos += nanos;
            c.allocations += ALLOCATIONS.load(Relaxed) - allocations;
            c.allocated += ALLOCATED.load(Relaxed) - allocated;
            c.bytes += raw.body.len() as u64;
            c.packets += 1;
        }
    }
    let mut rows: Vec<_> = costs.into_iter().collect();
    rows.sort_by_key(|(_, c)| std::cmp::Reverse(c.nanos));
    let total: u64 = rows.iter().map(|(_, c)| c.nanos).sum();
    println!("whole join: {:.2} ms", total as f64 / ROUNDS as f64 / 1e6);
    println!("{:<30} {:>6} {:>9} {:>9} {:>7} {:>9} {:>11}", "packet", "count", "bytes", "µs", "MB/s", "allocs", "alloc bytes");
    for (name, c) in rows.iter().take(14) {
        println!(
            "{:<30} {:>6} {:>9} {:>9.1} {:>7.0} {:>9} {:>11}",
            name,
            c.packets / ROUNDS,
            c.bytes / ROUNDS,
            c.nanos as f64 / ROUNDS as f64 / 1e3,
            c.bytes as f64 * 1e3 / c.nanos.max(1) as f64,
            c.allocations / ROUNDS,
            c.allocated / ROUNDS,
        );
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raws = join();
    let Some(name) = args.first() else {
        COUNTING.store(true, Relaxed);
        table(&raws);
        return Ok(());
    };
    let seconds = args.get(1).map_or(Ok(5.0), |s| s.parse::<f64>()).map_err(|e| e.to_string())?;
    let chosen: Vec<&RawPacket> = raws.iter().filter(|r| packet_name(r.id) == Some(name.as_str())).collect();
    if chosen.is_empty() {
        return Err(format!("the join has no {name}"));
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs_f64(seconds) {
        chosen.iter().for_each(|raw| drop(black_box(strict::check(raw))));
    }
    Ok(())
}
