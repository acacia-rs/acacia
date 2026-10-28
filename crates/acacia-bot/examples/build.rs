//! Live check of the schematic builder (acacia-pathfind/docs/builder.md): builds `SCENE` (default `house16`, the
//! acceptance build; small scenes in `support/house.rs`) on the pad from two barrel stations, with junk to clear.
//! The bot is disconnected after `KILL_AT` placements (default 150 for house16, else 0; 0: never) and a fresh one
//! resumes from the saved job. Afterwards an observer bot compares every cell with the structure. Needs the test
//! pack off.
//! `cargo run --release -p acacia-bot --example build -- <server> <name> <observer>`
mod support;

use std::time::Instant;

use acacia_bot::build::{BuildJob, BuildStatus, Builder, Phase};
use support::{house, Error};

/// Ticks to stay after the build: a cell BDS is still mining out (an unfinished break) only shows up later.
fn linger(scene: &str) -> u32 {
    env("LINGER", if scene == "house16" { 200 } else { 60 })
}

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name = args.next().unwrap_or_else(|| "Builder".into());
    let observer = args.next().unwrap_or_else(|| "BuildObs".into());
    let scene: String = env("SCENE", "house16".to_string());
    support::watchdog(if scene == "house16" { 900 } else { 240 });
    let kill_at: u32 = env("KILL_AT", if scene == "house16" { 150 } else { 0 });
    let (mut a, pad) = support::connect(&server, &name).await?;
    let job = house::setup(&mut a, &pad, &scene).await?;
    house::kit(&mut a, &pad).await?;
    pad.teleport(&mut a, [0.5, 0.0, 0.5], 180.0).await?;
    let saved = job.to_json();
    house::fresh_journals();
    let started = Instant::now();
    let (ticks, first, mut fixes) = house::drive(&mut a, house::journaled(Builder::new(job)?, &name)?, kill_at, &name).await?;
    let mut total = (ticks, first.clone());
    if kill_at > 0 && first.phase != Phase::Done {
        println!("killed after {ticks} ticks: {first:?}");
        a.disconnect().await;
        let (mut b, _) = support::connect(&server, &name).await?;
        let inv = b.state().inventory.main.iter().filter(|s| !s.is_empty()).count();
        println!("restarted with {inv} stacks in the inventory");
        for c in ["/give @s diamond_pickaxe", "/give @s diamond_shovel", "/give @s dirt 64"] {
            pad.run(&b, c);
            b.wait_ticks(4).await?;
        }
        let resumed = house::journaled(Builder::new(BuildJob::from_json(&saved)?)?, &format!("{name}-resumed"))?;
        let (more, second, f) = house::drive(&mut b, resumed, 0, &name).await?;
        fixes += f;
        total = (ticks + more, BuildStatus { placed: first.placed + second.placed, broken: first.broken + second.broken, restocks: first.restocks + second.restocks, ..second });
        pad.go_home(&mut b).await?;
        b.disconnect().await;
    } else {
        a.wait_ticks(linger(&scene)).await?;
        pad.go_home(&mut a).await?;
        a.disconnect().await;
    }
    let wrong = house::observe(&server, &observer, &pad, &BuildJob::from_json(&saved)?).await?;
    let (ticks, s) = total;
    println!(
        "{scene} {} after {ticks} ticks ({:.0} s): {} placed, {} broken, {} restocks, {fixes} corrections, {} mismatches, unsupported {:?}, failed {:?}, scaffold {}",
        if wrong.is_empty() && s.phase == Phase::Done { "PASS" } else { "FAIL" },
        started.elapsed().as_secs_f32(), s.placed, s.broken, s.restocks, wrong.len(), s.unsupported, s.failed, s.scaffold
    );
    wrong.iter().take(20).for_each(|m| println!("  {:?}: want {} found {}", m.pos, m.want, m.found));
    house::report(&wrong);
    Ok(())
}
