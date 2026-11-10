//! Walks a few ticks from rest at many arbitrary yaws, each from a fresh teleport, and prints the server
//! corrections per yaw and variant: isolates the heading from every other per-tick state.
//! `cargo run --release -p acacia-bot --example yaw_sweep -- <server> <name> [count] [variants] [ticks]`
//! `variants` is a comma list run in turn at every yaw (default `walk`), each made of words: `jitter` moves the
//! yaw by up to ±0.5° every tick, `airhold` only on ground ticks, `jump` sprint-jumps, `axis` snaps the yaw to
//! the nearest multiple of 90°. Offline bots rejoin where they left: run on a reset world, or the pad creeps up.
mod support;

use acacia_bot::movement::Controls;
use support::{Error, corrections};

/// Walk ticks per yaw; the first from rest shows the impulse alone in the server's velocity.
const WALK: u32 = 6;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "@default".into());
    let count: u32 = args.next().map_or(Ok(60), |s| s.parse())?;
    let variants: Vec<String> = args.next().unwrap_or_else(|| "walk".into()).split(',').map(str::to_owned).collect();
    let walk: u32 = args.next().map_or(Ok(WALK), |s| s.parse())?;
    let (mut bot, pad) = support::connect(&server, &name).await?;
    pad.build(&mut bot, &[], 5).await?;

    let mut seed: u32 = 0x9e37_79b9;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    // (yaws corrected, corrections) per variant
    let mut tally = vec![(0u32, 0u32); variants.len()];
    for _ in 0..count {
        let random = next() * 360.0 - 180.0;
        for (v, t) in variants.iter().zip(&mut tally) {
            let (jitter, jump, airhold) = (v.contains("jitter"), v.contains("jump"), v.contains("airhold"));
            let yaw = if v.contains("axis") { (random / 90.0).round() * 90.0 } else { random };
            pad.teleport(&mut bot, [10.5, 0.0, 0.5], yaw).await?;
            bot.wait_ticks(5).await?;
            bot.bedrock_mut().expect("Bedrock bot").trace_mark(&format!("{v} {yaw}"));
            let before = corrections(&bot);
            let mut y = yaw;
            for _ in 0..walk {
                let grounded = bot.bedrock().and_then(|b| b.movement()).is_some_and(|m| m.on_ground());
                if jitter && (!airhold || grounded) {
                    y = yaw + next() - 0.5;
                }
                if let Some(c) = bot.controls() {
                    *c = Controls { forward: 1.0, sprint: jump, jump, yaw: y, ..Controls::default() };
                }
                bot.wait_ticks(1).await?;
            }
            if let Some(c) = bot.controls() {
                *c = Controls { yaw: y, ..Controls::default() };
            }
            bot.wait_ticks(15).await?;
            let n = corrections(&bot) - before;
            t.0 += u32::from(n > 0);
            t.1 += n;
            println!("{v:16} yaw {yaw:12} corrections {n}");
        }
    }
    let feet_y = bot.bedrock().and_then(|b| b.movement()).and_then(|m| m.position()).map_or(f32::NAN, |p| p[1]);
    for (v, (hit, total)) in variants.iter().zip(&tally) {
        println!("{v:16} {hit} of {count} yaws corrected, {total} corrections (feet y {feet_y})");
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}
