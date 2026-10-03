//! Prints a tools/mitm capture one decoded packet per line: `t_ms dir name {Debug}` (t = ms since Login).
//!
//! `cargo run -p acacia-testserver --example capdump -- <capture.jsonl[#session]> [options]`
//!   --only A,B        only these packet names
//!   --exclude A,B     also hide these names (the noisy NOISY list is hidden unless --all)
//!   --all             do not hide NOISY
//!   --from MS --to MS time window, ms since Login
//!   --client | --server  one direction only
//!   --pai changes|all|none  PlayerAuthInput: ticks whose flags changed or that carry a transaction,
//!                     item stack request or block actions (default), every tick, or none
//!   --cap N           cut each line at N chars (default 1500, 0 = no cap)
use std::collections::BTreeSet;

use acacia_proto::Packet as _;
use acacia_proto::packets::{self, PlayerAuthInput};
use acacia_testserver::capture::{self, Packet};

/// High-volume packets that bury the actions; shown with --all or --only.
const NOISY: &[&str] = &[
    "MoveEntityDelta", "SetEntityMotion", "SetEntityData", "ClientboundMapItemData", "LevelChunk", "Subchunk",
    "SubchunkRequest", "ClientCacheMissResponse", "ClientCacheBlobStatus", "NetworkChunkPublisherUpdate",
    "CurrentStructureFeature", "SyncWorldClocks", "CorrectPlayerMovePrediction", "ContainerSetData",
];

/// PlayerAuthInput flags that flicker every few ticks without meaning a different activity.
const FLAG_NOISE: &[&str] = &["VerticalCollision", "HorizontalCollision", "CameraRelativeMovementEnabled", "BlockBreakingDelayEnabled"];

#[derive(PartialEq)]
enum PaiMode {
    Changes,
    All,
    None,
}

struct Opts {
    only: Option<BTreeSet<String>>,
    exclude: BTreeSet<String>,
    from: f64,
    to: f64,
    dir: Option<bool>,
    pai: PaiMode,
    cap: usize,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("usage: capdump <capture.jsonl[#n]> [--only A,B] [--exclude A,B] [--all] [--from ms] [--to ms] [--client|--server] [--pai changes|all|none] [--cap n]")?;
    let opts = parse(args.collect())?;
    let session = capture::session(&input)?;
    if let Some(s) = session.spawned {
        println!("# spawned at {s:.0} ms");
    }
    let mut last_flags = None;
    for p in &session.packets {
        let shown = keep(&opts, p);
        let line = if p.name == "PlayerAuthInput" {
            let Ok(pk) = PlayerAuthInput::decode(&mut &p.body[..]) else { continue };
            let flags: BTreeSet<String> = pk.input_data.iter().map(|f| format!("{f:?}")).filter(|f| !FLAG_NOISE.contains(&f.as_str())).collect();
            let changed = last_flags.as_ref() != Some(&flags);
            last_flags = Some(flags.clone());
            let busy = pk.transaction.is_some() || pk.item_stack_request.is_some() || pk.block_action.is_some();
            if !shown || opts.pai == PaiMode::None || (opts.pai == PaiMode::Changes && !changed && !busy) {
                continue;
            }
            pai_line(&pk, &flags)
        } else {
            if !shown {
                continue;
            }
            dump(p).unwrap_or_else(|| format!("<undecoded {} bytes>", p.body.len()))
        };
        println!("{:>9.0} {} {} {}", p.t, if p.from_client { "C>S" } else { "S>C" }, p.name, cap(line, opts.cap));
    }
    Ok(())
}

fn keep(o: &Opts, p: &Packet) -> bool {
    let named = match &o.only {
        Some(only) => only.contains(&p.name),
        None => !o.exclude.contains(&p.name),
    };
    named && p.t >= o.from && p.t <= o.to && o.dir.is_none_or(|c| c == p.from_client)
}

fn pai_line(pk: &PlayerAuthInput, flags: &BTreeSet<String>) -> String {
    let mut s = format!(
        "tick={} pos=({:.2},{:.2},{:.2}) rot=({:.1},{:.1}) mv=({:.2},{:.2}) flags={:?}",
        pk.tick, pk.position.x, pk.position.y, pk.position.z, pk.pitch, pk.yaw, pk.move_vector.x, pk.move_vector.z, flags
    );
    if let Some(t) = &pk.transaction {
        s += &format!(" transaction={t:?}");
    }
    if let Some(r) = &pk.item_stack_request {
        s += &format!(" item_stack_request={r:?}");
    }
    if let Some(b) = &pk.block_action {
        s += &format!(" block_action={b:?}");
    }
    if let Some(v) = &pk.vehicle_rotation {
        s += &format!(" vehicle_rotation=({:.1},{:.1}) predicted_vehicle={:?}", v.x, v.z, pk.predicted_vehicle);
    }
    s
}

fn cap(line: String, n: usize) -> String {
    if n == 0 || line.len() <= n {
        return line;
    }
    let cut = (0..=n).rev().find(|i| line.is_char_boundary(*i)).unwrap_or(0);
    format!("{}...(+{} chars)", &line[..cut], line.len() - cut)
}

fn parse(args: Vec<String>) -> Result<Opts, String> {
    let mut o = Opts { only: None, exclude: BTreeSet::new(), from: f64::MIN, to: f64::MAX, dir: None, pai: PaiMode::Changes, cap: 1500 };
    let mut all = false;
    let mut it = args.into_iter();
    let names = |s: Option<String>| -> Result<BTreeSet<String>, String> { Ok(s.ok_or("missing name list")?.split(',').map(str::to_owned).collect()) };
    let num = |s: Option<String>| -> Result<f64, String> { s.ok_or("missing number")?.parse::<f64>().map_err(|e| e.to_string()) };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--only" => o.only = Some(names(it.next())?),
            "--exclude" => o.exclude.extend(names(it.next())?),
            "--all" => all = true,
            "--from" => o.from = num(it.next())?,
            "--to" => o.to = num(it.next())?,
            "--client" => o.dir = Some(true),
            "--server" => o.dir = Some(false),
            "--cap" => o.cap = num(it.next())? as usize,
            "--pai" => {
                o.pai = match it.next().as_deref() {
                    Some("changes") => PaiMode::Changes,
                    Some("all") => PaiMode::All,
                    Some("none") => PaiMode::None,
                    other => return Err(format!("--pai {other:?}: expected changes|all|none")),
                }
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !all {
        o.exclude.extend(NOISY.iter().map(|s| s.to_string()));
    }
    Ok(o)
}

macro_rules! dumper {
    ($($t:ident),* $(,)?) => {
        /// The decoded packet as one-line Debug text.
        fn dump(p: &Packet) -> Option<String> {
            $(if p.id == <packets::$t>::ID {
                return Some(match <packets::$t>::decode(&mut &p.body[..]) {
                    Ok(v) => format!("{v:?}"),
                    Err(e) => format!("<decode error: {e}>"),
                });
            })*
            None
        }
    };
}
acacia_proto::for_each_packet!(dumper);
