//! Elytra autopilot drill on a strict server-authoritative BDS where the bot is operator: gives an elytra and
//! rockets, then `fly_to` a target `dx`, `dz` from the spawn; prints the corrections, ticks, rockets used and where
//! it landed (docs/pathfind-plan.md "M2 design").
//! `cargo run -p acacia-bot --example fly -- <server> <name|@account> <dx> <dz> [rockets]`
//! `BEDROCK_PROFILE=human` flies the Human profile.
mod support;

use acacia_bot::pathfind::flight::FlyOpts;
use acacia_bot::pathfind::MovementProfile;
use support::{corrections, Error};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    support::watchdog(600);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19184".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Flyer".into());
    let num = |i: usize, default: i32| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(default);
    let (dx, dz, rockets) = (num(2, 1000), num(3, 0), num(4, 64));
    let profile = match std::env::var("BEDROCK_PROFILE").as_deref() {
        Ok("human") => MovementProfile::Human,
        _ => MovementProfile::Optimal,
    };
    let (mut bot, _pad) = support::connect(&server, &name).await?;
    for cmd in ["/gamemode survival @s".to_string(), "/clear @s".into(), "/give @s elytra".into(), format!("/give @s firework_rocket {rockets}")] {
        bot.client().command(&cmd);
        bot.wait_ticks(4).await?;
    }
    bot.wait_ticks(20).await?;
    let start = bot.block_position().ok_or("no position")?;
    let target = [start[0] + dx, start[1], start[2] + dz];
    let (before, t0) = (corrections(&bot), std::time::Instant::now());
    let opts = FlyOpts { profile, ..FlyOpts::default() };
    let result = bot.fly_to(target, opts).await;
    let feet = bot.movement().and_then(|m| m.position()).ok_or("no position")?;
    let off = (feet[0] - (target[0] as f32 + 0.5)).hypot(feet[2] - (target[2] as f32 + 0.5));
    let secs = t0.elapsed().as_secs_f32();
    println!(
        "{profile:?} {result:?}: {} corrections, {secs:.1} s, landed {off:.2} from {target:?} at {feet:?}, rockets left {}",
        corrections(&bot) - before,
        bot.state().inventory.main.iter().filter(|s| !s.is_empty() && s.count > 0).map(|s| u32::from(s.count)).sum::<u32>()
    );
    Ok(())
}
