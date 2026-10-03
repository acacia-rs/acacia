//! Summarizes an acacia-mitm capture with acacia-proto: runs of ticks with the same input flags,
//! with speed, height and how many of those ticks the server corrected.
//! `cargo run -p acacia-client --example mitm_inspect -- .testserver/mitm/<capture>.jsonl`
//! `MITM_ALL_FLAGS=1` keeps the flags normally hidden as noise.
use std::collections::{BTreeSet, HashSet};

use acacia_client::proto::packets::{CorrectPlayerMovePrediction, PlayerAuthInput};

/// Flags that change every few ticks without meaning a different activity.
const NOISE: &[&str] = &["VerticalCollision", "HorizontalCollision", "CameraRelativeMovementEnabled", "BlockBreakingDelayEnabled"];

struct Tick {
    t: f64,
    tick: u64,
    flags: BTreeSet<String>,
    speed: f32,
    y: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: mitm_inspect <capture.jsonl>")?;
    let text = std::fs::read_to_string(path)?;
    let noise: &[&str] = if std::env::var_os("MITM_ALL_FLAGS").is_some() { &[] } else { NOISE };
    let mut ticks = Vec::new();
    let mut corrected = HashSet::new();
    for line in text.lines() {
        let v: serde_json::Value = serde_json::from_str(line)?;
        match v["name"].as_str() {
            Some("PlayerAuthInput") => {
                let raw = hex(v["raw"].as_str().unwrap_or_default());
                let pk = PlayerAuthInput::read(&mut &raw[..])?;
                let flags = pk.input_data.iter().map(|f| format!("{f:?}")).filter(|f| !noise.contains(&f.as_str())).collect();
                let speed = (pk.delta.x.powi(2) + pk.delta.z.powi(2)).sqrt() * 20.0;
                ticks.push(Tick { t: v["t"].as_f64().unwrap_or(0.0) / 1000.0, tick: pk.tick, flags, speed, y: pk.position.y });
            }
            Some("CorrectPlayerMovePrediction") => {
                let raw = hex(v["raw"].as_str().unwrap_or_default());
                corrected.insert(CorrectPlayerMovePrediction::read(&mut &raw[..])?.tick);
            }
            _ => {}
        }
    }
    let t0 = ticks.first().map_or(0.0, |t| t.t);
    println!("{:>7} {:>6} {:>5} {:>8} {:>8} {:>5}  flags", "time_s", "tick", "ticks", "speed", "y", "corr");
    for run in ticks.chunk_by(|a, b| a.flags == b.flags) {
        let n = run.len();
        let corr = run.iter().filter(|t| corrected.contains(&t.tick)).count();
        let speed = run.iter().map(|t| t.speed).sum::<f32>() / n as f32;
        let flags: Vec<&str> = run[0].flags.iter().map(String::as_str).collect();
        println!("{:>7.1} {:>6} {n:>5} {speed:>7.2}/s {:>8.2} {corr:>5}  {}", run[0].t - t0, run[0].tick, run[0].y, flags.join(","));
    }
    Ok(())
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}
