//! Compares what two clients sent in acacia-mitm captures, e.g. the vanilla game (left) against a bot
//! (right): packets missing on one side, order to spawn, reaction delays, send cadence and the fields of
//! one-off packets. Server packets only serve as reaction triggers.
//!
//! `cargo run -p acacia-capdiff -- <left.jsonl[#session]> <right.jsonl[#session]> [--window <s after spawn>]`
//! Sessions count from 0, negative from the end; default the last one in the file.
//! `capdiff pacing <left.datagrams.tsv> <right.datagrams.tsv>` compares RakNet send pacing (pacing.rs).

use acacia_testserver::capture;

mod diff;
mod pacing;
mod report;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [mode, left, right] = &args[..]
        && mode == "pacing"
    {
        return pacing::run(left, right);
    }
    let usage = "usage: capdiff <left.jsonl[#n]> <right.jsonl[#n]> [--window <seconds after spawn>]";
    let (Some(left), Some(right)) = (args.first(), args.get(1)) else { return Err(usage.into()) };
    let window = match args.get(2).map(String::as_str) {
        Some("--window") => args.get(3).and_then(|s| s.parse::<f64>().ok()).ok_or(usage)?,
        Some(_) => return Err(usage.into()),
        None => 15.0,
    };
    let (a, b) = (capture::session(left)?, capture::session(right)?);
    report::summary(&a, &b);
    report::inventory(&a, &b);
    report::order(&a, &b, window * 1000.0);
    report::reactions(&a, &b);
    report::cadence(&a, &b);
    report::fields(&a, &b);
    Ok(())
}
