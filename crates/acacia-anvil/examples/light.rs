//! Light only, on the scale harness's terrain converted once up front: `cargo run --release -p acacia-anvil
//! --example light -- [side=32] [iters=5] [mode=both|area|column]`. One thread; prints columns/s, heap
//! allocations per column and a hash of every extracted light array (equal hashes = identical light).
#[allow(dead_code)]
#[path = "scale/terrain.rs"]
mod terrain;

use std::alloc::{GlobalAlloc, Layout, System};
use std::hash::{DefaultHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};

use acacia_anvil::column::{Converter, JavaColumn, Scratch};
use acacia_anvil::light::{Lighter, Neighbourhood, SectionLight};
use acacia_anvil::props::PropsTable;
use acacia_world::BlockRegistry;

struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const GROUP: i32 = 8;

struct World {
    side: i32,
    cols: Vec<JavaColumn>,
}

impl World {
    fn get(&self, x: i32, z: i32) -> Option<&JavaColumn> {
        ((0..self.side).contains(&x) && (0..self.side).contains(&z))
            .then(|| &self.cols[(z * self.side + x) as usize])
    }
}

fn hash_light(h: &mut DefaultHasher, light: &[SectionLight]) {
    for s in light {
        h.write(&s.sky[..]);
        h.write(&s.block[..]);
    }
}

/// Aligned 8×8 groups over one volume padded by a column, as `store/write.rs` lights them.
fn run_area(w: &World, l: &mut Lighter, p: &PropsTable, h: &mut DefaultHasher) -> [Duration; 2] {
    let (mut light, mut extract) = (Duration::ZERO, Duration::ZERO);
    let mut area = Vec::new();
    for gz in (0..w.side).step_by(GROUP as usize) {
        for gx in (0..w.side).step_by(GROUP as usize) {
            let (x1, z1) = ((gx + GROUP).min(w.side), (gz + GROUP).min(w.side));
            area.clear();
            area.extend((gz - 1..=z1).flat_map(|z| (gx - 1..=x1).map(move |x| (x, z))).map(|(x, z)| w.get(x, z)));
            let t = Instant::now();
            l.light_area(&area, (x1 - gx + 2) as usize, p, true);
            let t2 = Instant::now();
            light += t2 - t;
            for z in gz..z1 {
                for x in gx..x1 {
                    let col = w.get(x, z).unwrap();
                    let out = l.extract((x - gx + 1) as usize, (z - gz + 1) as usize, col.sections.len());
                    hash_light(h, &out);
                }
            }
            extract += t2.elapsed();
        }
    }
    [light, extract]
}

fn run_column(w: &World, l: &mut Lighter, p: &PropsTable, h: &mut DefaultHasher) -> [Duration; 2] {
    let t = Instant::now();
    for z in 0..w.side {
        for x in 0..w.side {
            let mut cols = [None; 9];
            for (i, c) in cols.iter_mut().enumerate() {
                *c = w.get(x + i as i32 % 3 - 1, z + i as i32 / 3 - 1);
            }
            hash_light(h, &l.light(&Neighbourhood { cols }, p, true));
        }
    }
    [t.elapsed(), Duration::ZERO]
}

type Run = fn(&World, &mut Lighter, &PropsTable, &mut DefaultHasher) -> [Duration; 2];

fn main() {
    let (mut side, mut iters, mut mode) = (32i32, 5u32, "both".to_string());
    for arg in std::env::args().skip(1) {
        match arg.split_once('=') {
            Some(("side", v)) => side = v.parse().expect("side"),
            Some(("iters", v)) => iters = v.parse().expect("iters"),
            Some(("mode", v)) => mode = v.to_string(),
            _ => panic!("unknown argument {arg}"),
        }
    }
    let reg = BlockRegistry::vanilla_arc();
    let terrain = terrain::Terrain::new(&reg);
    let conv = Converter::new(&reg);
    let mut scratch = Scratch::default();
    let cols = (0..side * side)
        .map(|i| conv.convert(&terrain.chunk(i % side, i / side, (i % 3) as u32), &mut scratch))
        .collect();
    let world = World { side, cols };
    let props = PropsTable::new();
    let n = (side * side) as f64 * iters as f64;
    let runs: [(&str, Run); 2] = [("area", run_area), ("column", run_column)];
    for (name, run) in runs {
        if mode != "both" && mode != name {
            continue;
        }
        let mut lighter = Lighter::default();
        let mut h = DefaultHasher::new();
        run(&world, &mut lighter, &props, &mut h);
        let allocs = ALLOCS.load(Relaxed);
        let mut total = [Duration::ZERO; 2];
        for _ in 0..iters {
            let d = run(&world, &mut lighter, &props, &mut h);
            total = [total[0] + d[0], total[1] + d[1]];
        }
        let allocs = ALLOCS.load(Relaxed) - allocs;
        println!(
            "{name}: {:.0} columns/s light, {:.0}/s with extract, {:.1} allocations/column, hash {:016x}",
            n / total[0].as_secs_f64(),
            n / (total[0] + total[1]).as_secs_f64(),
            allocs as f64 / n,
            h.finish()
        );
    }
}
