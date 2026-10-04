//! Replays a recorded movement trace offline and reports, per mark (drill), the ticks that no longer
//! match the recording and the server corrections the replayed simulation disagrees with.
//! `cargo run -p acacia-bot --example replay -- <trace> [--resync] [--tolerance 0.0001] [--correction-tolerance 0.001] [--verbose]`
use std::collections::BTreeMap;
use std::path::PathBuf;

use acacia_bot::trace::{self, CORRECTION_TOLERANCE};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).without_time().with_ansi(false).try_init();
    let mut path = None;
    let (mut resync, mut verbose, mut tolerance, mut correction_tolerance) = (false, false, 1e-4, CORRECTION_TOLERANCE);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--resync" => resync = true,
            "--verbose" => verbose = true,
            "--tolerance" => tolerance = args.next().ok_or("--tolerance needs a value")?.parse()?,
            "--correction-tolerance" => correction_tolerance = args.next().ok_or("--correction-tolerance needs a value")?.parse()?,
            _ => path = Some(PathBuf::from(a)),
        }
    }
    let events = trace::read(&path.ok_or("usage: replay <trace> [--resync] [--tolerance x] [--verbose]")?)?;
    let report = trace::replay(&events, tolerance, resync);

    let mut per_mark: BTreeMap<String, (usize, usize, f32)> = BTreeMap::new();
    for d in &report.diverged {
        let e = per_mark.entry(label(&d.mark)).or_default();
        e.0 += 1;
        e.2 = e.2.max(dist(d.ours, d.recorded));
    }
    let (mismatches, lagged) = report.correction_mismatches(correction_tolerance);
    for c in &mismatches {
        per_mark.entry(label(&c.mark)).or_default().1 += 1;
    }
    println!(
        "{} ticks replayed, {} diverged from the recording ({} near a knockback or teleport), {} of {} corrections mismatch, {} teleport lags skipped",
        report.ticks,
        report.diverged.len(),
        report.diverged.iter().filter(|d| d.near_event).count(),
        mismatches.len(),
        report.corrections.len(),
        lagged.len()
    );
    for (mark, (diverged, corrections, max)) in &per_mark {
        println!("  {mark:20} diverged {diverged:4} (max {max:.5})  correction mismatches {corrections}");
    }
    if verbose {
        for d in &report.diverged {
            let event = if d.near_event { " (event)" } else { "" };
            println!("tick {:6} {:16} ours {:?} recorded {:?} delta err {:.5}{event} [{}]", d.tick, label(&d.mark), d.ours, d.recorded, d.delta_err, d.blocks);
        }
        for c in &mismatches {
            println!("corr {:6} {:16} ours {:?} server {:?} err {:?} delta err {:?} [{}]", c.tick, label(&c.mark), c.ours, c.server, c.error(), c.delta_err, c.blocks);
            if let (Some((pos, delta)), Some(ours)) = (c.sent, c.ours) {
                println!("     sent {pos:?} {delta:?}: client-server {:.5}, ours-client {:.5}", dist(pos, c.server), dist(ours, pos));
            }
        }
        for c in &lagged {
            println!("lag  {:6} {:16} ours {:?} server {:?}", c.tick, label(&c.mark), c.ours, c.server);
        }
    }
    Ok(())
}

fn label(mark: &Option<String>) -> String {
    mark.clone().unwrap_or_else(|| "(start)".into())
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
