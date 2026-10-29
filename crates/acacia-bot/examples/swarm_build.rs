//! Live check of the shared build (acacia-pathfind/docs/builder.md "Swarm"): house16 by `BOTS` builders (1 or 2,
//! default 2) in one process sharing a board. With `KILL_AT` > 0 the second bot is dropped after that many
//! placements and never comes back; the first must finish its regions too. Afterwards an observer compares every
//! cell with the structure. Needs the test pack off.
//! `cargo run --release -p acacia-bot --example swarm_build -- <server> <name a> <name b> <observer>`
mod support;

use std::sync::Arc;
use std::time::Instant;

use acacia_bot::build::{BuildStatus, Builder, LocalBoard, Phase, TaskBoard};
use acacia_bot::Bot;
use support::{house, Error, Pad};

/// Board lease: a dropped bot's region returns this long after its last heartbeat.
const LEASE_MS: u64 = 10_000;

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

type Run = (u32, BuildStatus, u32);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let (na, nb) = (args.next().unwrap_or_else(|| "BuildA".into()), args.next().unwrap_or_else(|| "BuildB".into()));
    let observer = args.next().unwrap_or_else(|| "BuildObs".into());
    let (bots, kill_at): (u32, u32) = (env("BOTS", 2), env("KILL_AT", 0));
    support::watchdog(900);
    let (mut a, pad) = support::connect(&server, &na).await?;
    let job = house::setup(&mut a, &pad, &env("SCENE", "house16".to_string())).await?;
    house::kit(&mut a, &pad).await?;
    pad.teleport(&mut a, [0.5, 0.0, 0.5], 180.0).await?;
    // `WATCH=<player>`: a player already on the server (a viewer) watches from a glass stand above the house.
    // Not as a spectator: the viewer does not simulate spectator flight, so the server snapped it back ~4 times a
    // second.
    if let Ok(watcher) = std::env::var("WATCH") {
        let ([x, y, z], [fx, fy, fz]) = (pad.block([12, 14, -16]), pad.block([12, 0, 0]));
        let stand = format!("/fill {} {} {} {} {} {} glass", x - 1, y - 1, z - 1, x + 1, y - 1, z + 1);
        for c in [format!("/gamemode survival {watcher}"), stand, format!("/tp {watcher} {x} {y} {z} facing {fx} {fy} {fz}")] {
            pad.run(&a, &c);
            a.wait_ticks(4).await?;
        }
    }
    house::fresh_journals();
    let started = Instant::now();
    let (ra, rb) = if bots < 2 {
        (house::drive(&mut a, house::journaled(Builder::new(job.clone())?, &na)?, 0, &na).await?, None)
    } else {
        let (mut b, pb) = support::connect(&server, &nb).await?;
        let pb = pb.at(pad.origin());
        house::kit(&mut b, &pb).await?;
        pb.teleport(&mut b, [2.5, 0.0, 0.5], 180.0).await?;
        let board: Arc<dyn TaskBoard> = Arc::new(LocalBoard::new(LEASE_MS));
        let ba = house::journaled(Builder::new(job.clone())?.with_board(board.clone(), na.as_str()), &na)?;
        let bb = house::journaled(Builder::new(job.clone())?.with_board(board, nb.as_str()), &nb)?;
        let (ra, rb) = tokio::join!(house::drive(&mut a, ba, 0, &na), second(b, &pb, bb, kill_at, &nb));
        (ra?, Some(rb?))
    };
    let secs = started.elapsed().as_secs_f32();
    pad.go_home(&mut a).await?;
    a.disconnect().await;
    let wrong = house::observe(&server, &observer, &pad, &job).await?;
    let done = ra.1.phase == Phase::Done || rb.as_ref().is_some_and(|r| r.1.phase == Phase::Done);
    let ticks = ra.0.max(rb.as_ref().map_or(0, |r| r.0));
    let line = |n: &str, r: &Run| format!("{n}: {} ticks, {} placed, {} broken, {} restocks, {} walks, {} corrections, {:?}", r.0, r.1.placed, r.1.broken, r.1.restocks, r.1.walks, r.2, r.1.phase);
    println!("{}", line(&na, &ra));
    if let Some(rb) = &rb {
        println!("{}", line(&nb, rb));
    }
    println!("{} with {bots} bot(s): {ticks} ticks ({secs:.0} s), {} mismatches", if wrong.is_empty() && done { "PASS" } else { "FAIL" }, wrong.len());
    wrong.iter().take(20).for_each(|m| println!("  {:?}: want {} found {}", m.pos, m.want, m.found));
    house::report(&wrong);
    Ok(())
}

/// The second bot: dropped for good after `kill_at` placements (0: builds to the end).
async fn second(mut b: Bot, pad: &Pad, builder: Builder, kill_at: u32, name: &str) -> Result<Run, Error> {
    let r = house::drive(&mut b, builder, kill_at, name).await?;
    if r.1.phase != Phase::Done && kill_at > 0 {
        println!("{name} dropped after {} ticks, {} placed", r.0, r.1.placed);
    } else {
        pad.go_home(&mut b).await?;
    }
    b.disconnect().await;
    Ok(r)
}
