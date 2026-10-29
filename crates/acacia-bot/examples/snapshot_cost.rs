//! What publishing a per-tick `BotSnapshot` for a JS binding costs: build time, bytes, allocations, the
//! publish into a shared slot, and JSON (research/js-measure/snapshot-results.md).
//! `cargo run --release -p acacia-bot --example snapshot_cost` prints the table.
//! `... -- run <bench> <iters>` runs one bench alone for `perf stat -e instructions:u` (`none` = baseline).

#[path = "snapshot_cost/fixture.rs"]
mod fixture;
#[path = "snapshot_cost/snapshot.rs"]
mod snapshot;

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use acacia_bot::GameState;
use arc_swap::ArcSwap;
use snapshot::{BotSnapshot, Cold, Hot, Motion, build_cold, build_full, build_hot, cold_heap};

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED: AtomicU64 = AtomicU64::new(0);
/// Off while timing: the atomic adds would be part of every measured allocation.
static COUNTING: AtomicBool = AtomicBool::new(false);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCATIONS.fetch_add(1, Relaxed);
            ALLOCATED.fetch_add(layout.size() as u64, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Relaxed) {
            ALLOCATIONS.fetch_add(1, Relaxed);
            ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()) as u64, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const ITERS: u64 = 1_000_000;
const RUNS: usize = 7;
/// Cold part republished one tick in this many in the split variants (inventory/player churn).
const COLD_EVERY: u64 = 20;
const TICK_HZ: f64 = 20.0;

const BENCHES: &[&str] = &[
    "build_hot",
    "build_cold_ids",
    "build_cold_names",
    "build_full",
    "full_arcswap",
    "full_plmutex",
    "full_stdmutex",
    "hot_arcswap",
    "hot_plmutex_copy",
    "split_arcswap",
    "split_plmutex",
    "json_hot",
    "json_full",
    "read_arcswap",
    "read_plmutex",
];

struct Ctx {
    state: GameState,
    motion: Motion,
    full_swap: ArcSwap<BotSnapshot>,
    full_pl: parking_lot::Mutex<Arc<BotSnapshot>>,
    full_std: Mutex<Arc<BotSnapshot>>,
    hot_swap: ArcSwap<Hot>,
    hot_pl: parking_lot::Mutex<Hot>,
    cold_swap: ArcSwap<Cold>,
    cold_pl: parking_lot::Mutex<Arc<Cold>>,
    json: Vec<u8>,
}

fn time(iters: u64, mut f: impl FnMut(u64)) -> f64 {
    let start = Instant::now();
    for i in 0..iters {
        f(i);
    }
    start.elapsed().as_nanos() as f64 / iters as f64
}

fn swap_pl<T>(slot: &parking_lot::Mutex<Arc<T>>, new: Arc<T>) {
    let old = std::mem::replace(&mut *slot.lock(), new);
    drop(old);
}

fn swap_std<T>(slot: &Mutex<Arc<T>>, new: Arc<T>) {
    let old = std::mem::replace(&mut *slot.lock().unwrap(), new);
    drop(old);
}

/// ns per iteration of bench `name`; `None` for an unknown name.
fn bench(name: &str, c: &mut Ctx, iters: u64) -> Option<f64> {
    let (s, m) = (&c.state, &c.motion);
    let ns = match name {
        "none" => 0.0,
        "build_hot" => time(iters, |i| _ = black_box(build_hot(black_box(s), m, i))),
        "build_cold_ids" => time(iters, |_| _ = black_box(build_cold(black_box(s), false))),
        "build_cold_names" => time(iters, |_| _ = black_box(build_cold(black_box(s), true))),
        "build_full" => time(iters, |i| _ = black_box(build_full(black_box(s), m, i))),
        "full_arcswap" => time(iters, |i| c.full_swap.store(Arc::new(build_full(black_box(s), m, i)))),
        "full_plmutex" => time(iters, |i| swap_pl(&c.full_pl, Arc::new(build_full(black_box(s), m, i)))),
        "full_stdmutex" => time(iters, |i| swap_std(&c.full_std, Arc::new(build_full(black_box(s), m, i)))),
        "hot_arcswap" => time(iters, |i| c.hot_swap.store(Arc::new(build_hot(black_box(s), m, i)))),
        "hot_plmutex_copy" => time(iters, |i| *c.hot_pl.lock() = build_hot(black_box(s), m, i)),
        "split_arcswap" => time(iters, |i| {
            c.hot_swap.store(Arc::new(build_hot(black_box(s), m, i)));
            if i % COLD_EVERY == 0 {
                c.cold_swap.store(Arc::new(build_cold(s, false)));
            }
        }),
        "split_plmutex" => time(iters, |i| {
            *c.hot_pl.lock() = build_hot(black_box(s), m, i);
            if i % COLD_EVERY == 0 {
                swap_pl(&c.cold_pl, Arc::new(build_cold(s, false)));
            }
        }),
        "json_hot" => {
            let hot = build_hot(s, m, 1);
            time(iters, |_| {
                c.json.clear();
                serde_json::to_writer(&mut c.json, black_box(&hot)).unwrap();
            })
        }
        "json_full" => {
            let full = build_full(s, m, 1);
            time(iters, |_| {
                c.json.clear();
                serde_json::to_writer(&mut c.json, black_box(&full)).unwrap();
            })
        }
        "read_arcswap" => time(iters, |_| _ = black_box(c.full_swap.load_full())),
        "read_plmutex" => time(iters, |_| _ = black_box(c.full_pl.lock().clone())),
        _ => return None,
    };
    Some(ns)
}

/// (allocations, allocated bytes) per iteration, over a short counted pass.
fn allocations(name: &str, c: &mut Ctx) -> (f64, f64) {
    const N: u64 = 10_000;
    let (a, b) = (ALLOCATIONS.load(Relaxed), ALLOCATED.load(Relaxed));
    COUNTING.store(true, Relaxed);
    bench(name, c, N);
    COUNTING.store(false, Relaxed);
    ((ALLOCATIONS.load(Relaxed) - a) as f64 / N as f64, (ALLOCATED.load(Relaxed) - b) as f64 / N as f64)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn context() -> Ctx {
    let built = fixture::game_state();
    let state = built.state;
    println!(
        "fixture: {} script packets ({} failed to apply), {} stacks, {} players, {} effects, {} item ids",
        built.script_packets,
        built.script_errors,
        state.inventory.iter().count(),
        state.player_list.len(),
        state.player.effects.iter().count(),
        state.items.len()
    );
    let motion = Motion { velocity: [0.1, -0.0784, 0.2], on_ground: true, gliding: false };
    let full = Arc::new(build_full(&state, &motion, 0));
    let cold = Arc::new(full.cold.clone());
    Ctx {
        full_swap: ArcSwap::new(full.clone()),
        full_pl: parking_lot::Mutex::new(full.clone()),
        full_std: Mutex::new(full),
        hot_swap: ArcSwap::new(Arc::new(build_hot(&state, &motion, 0))),
        hot_pl: parking_lot::Mutex::new(build_hot(&state, &motion, 0)),
        cold_swap: ArcSwap::new(cold.clone()),
        cold_pl: parking_lot::Mutex::new(cold),
        json: Vec::with_capacity(16 << 10),
        state,
        motion,
    }
}

fn sizes(c: &Ctx) {
    let hot = build_hot(&c.state, &c.motion, 1);
    let (ids, names) = (build_cold(&c.state, false), build_cold(&c.state, true));
    println!(
        "bytes: Hot {} inline; Cold {} inline + {} heap (ids) / {} heap (names); BotSnapshot {} inline",
        size_of::<Hot>(),
        size_of::<Cold>(),
        cold_heap(&ids),
        cold_heap(&names),
        size_of::<BotSnapshot>()
    );
    println!(
        "json: hot {} B, cold(ids) {} B, cold(names) {} B, full {} B",
        json(&hot),
        json(&ids),
        json(&names),
        json(&BotSnapshot { hot, cold: ids.clone() })
    );
}

fn json<T: serde::Serialize>(v: &T) -> usize {
    serde_json::to_vec(v).unwrap().len()
}

fn table(c: &mut Ctx) {
    sizes(c);
    println!("{} runs x {ITERS} iterations; split = hot every tick + cold 1 tick in {COLD_EVERY}", RUNS);
    println!("{:<18} {:>9} {:>9} {:>8} {:>10} {:>8} {:>8} {:>8}", "bench", "median ns", "min ns", "allocs", "alloc B", "100 bots", "300", "1000");
    for name in BENCHES {
        let runs: Vec<f64> = (0..RUNS).map(|_| bench(name, c, ITERS).unwrap()).collect();
        let min = runs.iter().copied().fold(f64::INFINITY, f64::min);
        let med = median(runs);
        let (allocs, bytes) = allocations(name, c);
        let core = |bots: f64| format!("{:.4}%", med * TICK_HZ * bots / 1e9 * 100.0);
        println!(
            "{name:<18} {med:>9.1} {min:>9.1} {allocs:>8.2} {bytes:>10.0} {:>8} {:>8} {:>8}",
            core(100.0),
            core(300.0),
            core(1000.0)
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut c = context();
    match args.first().map(String::as_str) {
        Some("run") => {
            let name = args.get(1).map_or("none", String::as_str);
            let iters = args.get(2).map_or(ITERS, |n| n.parse().expect("iterations"));
            let ns = bench(name, &mut c, iters).unwrap_or_else(|| panic!("unknown bench {name}; one of {BENCHES:?} or none"));
            println!("{name}: {iters} iterations, {ns:.1} ns each");
        }
        _ => table(&mut c),
    }
}
