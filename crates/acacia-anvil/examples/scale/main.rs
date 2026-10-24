//! Scale test of the store under a bot-like crawl: `cargo run --release -p acacia-anvil --example scale --
//! [key=value…]` (defaults in `Args::default`). `bots` bots mow equal bands of a `side`² chunk world with
//! a `radius` view at `speed` chunks per second, relog every `relog` s (whole view sent again, unchanged),
//! and players edit `edits` chunks per second. Time runs `scale`× real time (the store's `max_wait` and
//! `region_interval` (`interval`), `region_hold` (`hold`) are divided by it). Prints memory, throughput, time breakdown, bytes written, and with
//! `check=<n>` compares n regions against an all-in-memory conversion.
mod bots;
mod check;
mod terrain;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use acacia_anvil::{AnvilStore, StoreConfig, StoreStats};
use acacia_world::BlockRegistry;
use bots::{Field, Pos};
use terrain::{Terrain, mix};

struct Args {
    side: i32,
    bots: i32,
    radius: i32,
    stride: i32,
    speed: f64,
    scale: f64,
    memory_mb: usize,
    interval: f64,
    hold: f64,
    wait: f64,
    relog: f64,
    edits: f64,
    report: f64,
    check: usize,
    threads: usize,
    out: PathBuf,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            side: 256,
            bots: 8,
            radius: 8,
            stride: 15,
            speed: 1.4,
            scale: 30.0,
            memory_mb: 1024,
            interval: 10.0,
            hold: 60.0,
            wait: 10.0,
            relog: 900.0,
            edits: 0.5,
            report: 600.0,
            check: 0,
            threads: 0,
            out: PathBuf::from("target/scale"),
        }
    }
}

fn num<T: std::str::FromStr>(v: &str, arg: &str) -> Result<T, String> {
    v.parse().map_err(|_| format!("bad value: {arg}"))
}

fn parse() -> Result<Args, String> {
    let mut a = Args::default();
    for arg in std::env::args().skip(1) {
        let (k, v) = arg.split_once('=').ok_or(format!("expected key=value: {arg}"))?;
        match k {
            "side" => a.side = num(v, &arg)?,
            "bots" => a.bots = num(v, &arg)?,
            "radius" => a.radius = num(v, &arg)?,
            "stride" => a.stride = num(v, &arg)?,
            "speed" => a.speed = num(v, &arg)?,
            "scale" => a.scale = num(v, &arg)?,
            "memory" => a.memory_mb = num(v, &arg)?,
            "interval" => a.interval = num(v, &arg)?,
            "hold" => a.hold = num(v, &arg)?,
            "wait" => a.wait = num(v, &arg)?,
            "relog" => a.relog = num(v, &arg)?,
            "edits" => a.edits = num(v, &arg)?,
            "report" => a.report = num(v, &arg)?,
            "check" => a.check = num(v, &arg)?,
            "threads" => a.threads = num(v, &arg)?,
            "out" => a.out = v.into(),
            _ => return Err(format!("unknown key: {k}")),
        }
    }
    Ok(a)
}

fn dir_bytes(dir: &std::path::Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|d| d.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

const MB: f64 = 1048576.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = parse()?;
    let region_dir = a.out.join("world/region");
    let _ = std::fs::remove_dir_all(&a.out);
    let reg = BlockRegistry::vanilla_arc();
    let terrain = Terrain::new(&reg);
    let secs = |s: f64| Duration::from_secs_f64(s / a.scale);
    let mut store = AnvilStore::new(
        reg.clone(),
        StoreConfig {
            region_dir: region_dir.clone(),
            has_sky: true,
            max_wait: secs(a.wait),
            region_interval: secs(a.interval),
            region_hold: secs(a.hold),
            memory_limit: a.memory_mb << 20,
            threads: a.threads,
        },
    )?;
    let field = Field { side: a.side, radius: a.radius, stride: a.stride };
    let mut bots = field.bots(a.bots);
    let tick = 0.5;
    let (mut vt, mut next_report, mut edit_carry) = (0.0f64, a.report, 0.0f64);
    let mut rng = 1u64;
    let mut revs: HashMap<Pos, u32> = HashMap::new();
    let mut seen = vec![0u64; (a.side as usize * a.side as usize).div_ceil(64)];
    let (mut unique, mut converted, mut lag_ticks) = (0u64, 0u64, 0u64);
    let (mut t_gen, mut t_update, mut t_flush) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let mut sent: Vec<Pos> = Vec::new();
    for b in &bots {
        field.view(b.at, &mut sent);
    }
    let start = Instant::now();
    println!("vt_s real_s unique converted cols/s held heap_MB rss_MB peak_MB dirty stale regions_known debounced writes written_GB read_back lag");
    loop {
        for (i, b) in bots.iter_mut().enumerate() {
            field.advance(b, a.speed * tick, &mut sent);
            let phase = a.relog * i as f64 / a.bots as f64;
            if !b.done && ((vt + phase) / a.relog).floor() != ((vt + phase + tick) / a.relog).floor() {
                field.view(b.at, &mut sent);
            }
        }
        edit_carry += a.edits * tick;
        while edit_carry >= 1.0 {
            edit_carry -= 1.0;
            rng = mix(rng);
            let b = &bots[(rng % a.bots as u64) as usize];
            let span = (2 * a.radius + 1) as u64;
            let p = (
                (b.at.0 + ((rng >> 16) % span) as i32 - a.radius).clamp(0, a.side - 1),
                (b.at.1 + ((rng >> 32) % span) as i32 - a.radius).clamp(0, a.side - 1),
            );
            *revs.entry(p).or_default() += 1;
            sent.push(p);
        }
        for p in sent.drain(..) {
            let i = (p.1 * a.side + p.0) as usize;
            if seen[i >> 6] & (1 << (i & 63)) == 0 {
                seen[i >> 6] |= 1 << (i & 63);
                unique += 1;
            }
            let t = Instant::now();
            let chunk = terrain.chunk(p.0, p.1, revs.get(&p).copied().unwrap_or(0));
            let t2 = Instant::now();
            store.update(&chunk, []);
            t_gen += t2 - t;
            t_update += t2.elapsed();
            converted += 1;
        }
        let t = Instant::now();
        store.flush(false)?;
        t_flush += t.elapsed();
        vt += tick;
        let done = bots.iter().all(|b| b.done);
        if vt >= next_report || done {
            next_report += a.report;
            let (rss, peak) = check::memory();
            let (c, s, real) = (store.census(), store.stats(), start.elapsed().as_secs_f64());
            println!(
                "{vt:.0} {real:.1} {unique} {converted} {:.0} {} {:.0} {:.0} {:.0} {} {} {} {} {} {:.2} {} {lag_ticks}",
                unique as f64 / real, c.columns, store.heap_bytes() as f64 / MB, rss as f64 / MB,
                peak as f64 / MB, c.dirty, c.stale, c.regions_known, c.regions_debounced, s.region_writes,
                s.bytes_written as f64 / MB / 1024.0, s.read_back_columns
            );
        }
        if done {
            break;
        }
        let target = Duration::from_secs_f64(vt / a.scale);
        match target.checked_sub(start.elapsed()) {
            Some(ahead) => std::thread::sleep(ahead),
            None => lag_ticks += 1,
        }
    }
    let t = Instant::now();
    store.flush(true)?;
    t_flush += t.elapsed();
    let real = start.elapsed().as_secs_f64();
    summary(&a, store.stats(), unique, converted, real, [t_gen, t_update, t_flush], dir_bytes(&region_dir));
    if a.check > 0 {
        let n = (a.side + 31) / 32;
        let picks: Vec<(i32, i32)> = (0..a.check as u64)
            .map(|k| ((mix(k + 11) % n as u64) as i32, (mix(k + 97) % n as u64) as i32))
            .collect();
        let (compared, differ) = check::regions(
            &reg, &terrain, &revs, a.side, &region_dir, &a.out.join("check"), &picks,
        )?;
        println!("check {picks:?}: {compared} chunks compared, {differ} differ");
    }
    Ok(())
}

fn summary(a: &Args, s: &StoreStats, unique: u64, converted: u64, real: f64, t: [Duration; 3], disk: u64) {
    let ms = |d: Duration| format!("{:.1}s", d.as_secs_f64());
    let (_, peak) = check::memory();
    println!(
        "side {} ({} blocks), {} bots r{} {} ch/s, scale {}, memory {} MB, interval {} s, hold {} s, wait {} s",
        a.side, a.side * 16, a.bots, a.radius, a.speed, a.scale, a.memory_mb, a.interval, a.hold, a.wait
    );
    println!(
        "{unique} chunks ({converted} conversions) in {real:.1}s: {:.0} unique/s wall, {:.0}/s of store time; peak working set {:.0} MB",
        unique as f64 / real,
        unique as f64 / (t[1] + t[2]).as_secs_f64(),
        peak as f64 / MB
    );
    println!(
        "time: gen {} update {} flush {} = read_back {} ({} cols, decode {}) region_load {} ({:.1} GB) fixup {} render {} (threads: light {} encode {} compress {}) save {} evict {}",
        ms(t[0]), ms(t[1]), ms(t[2]), ms(s.read_back), s.read_back_columns, ms(s.read_back_decode), ms(s.region_load),
        s.region_load_bytes as f64 / MB / 1024.0, ms(s.fixup), ms(s.render_wall), ms(s.light),
        ms(s.encode), ms(s.compress), ms(s.save), ms(s.evict)
    );
    println!(
        "writes: {} region files, {:.2} GB, {} chunk slots replaced, {} relit unchanged; on disk {:.2} GB ({:.0} B/chunk); {:.0} B written per chunk, amplification {:.1}x",
        s.region_writes, s.bytes_written as f64 / MB / 1024.0, s.chunks_written, s.chunks_unchanged,
        disk as f64 / MB / 1024.0, disk as f64 / unique as f64, s.bytes_written as f64 / unique as f64,
        s.bytes_written as f64 / disk as f64
    );
}
