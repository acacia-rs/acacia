//! Physics bot: waits, walks a square (with a jump), then idles, printing position and how often
//! the server corrected or teleported it (0 corrections = our simulation matched the server).
//! `cargo run -p acacia-bot --features socks --example walk -- <server> <name|@account>`
//! `BEDROCK_PROXY=host:port:user:pass` as in the other examples.
use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::{Client, Socks5Proxy};
use acacia_bot::{Bot, BotConfig, BotEvent};

/// (seconds, forward, strafe, jump, yaw) phases.
const PLAN: &[(u64, f32, f32, bool, f32)] = &[
    (3, 0.0, 0.0, false, 0.0),
    (2, 1.0, 0.0, false, 0.0),
    (2, 1.0, 0.0, false, 90.0),
    (1, 1.0, 0.0, true, 180.0),
    (2, 1.0, 0.0, false, 180.0),
    (2, 1.0, 0.0, false, -90.0),
    (5, 0.0, 0.0, false, -90.0),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Walker".into());
    let proxy = std::env::var("BEDROCK_PROXY").ok().map(|p| Socks5Proxy::parse(&p)).transpose()?;

    let mut builder = Client::builder(&server);
    if let Some(p) = &proxy {
        builder = builder.proxy(p.clone());
    }
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let config = AuthConfig { proxy: proxy.as_ref().map(Socks5Proxy::to_url), ..AuthConfig::default() };
            let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(&name),
    };
    let mut bot = Bot::connect(builder, BotConfig { physics: true, ..BotConfig::default() }).await?;
    println!("spawned as {}", bot.client().display_name());

    let mut report = tokio::time::interval(Duration::from_secs(1));
    let mut phase = 0usize;
    let mut phase_end = tokio::time::Instant::now() + Duration::from_secs(PLAN[0].0);
    loop {
        tokio::select! {
            event = bot.next() => match event {
                Some(BotEvent::Disconnected(r)) => { println!("disconnected: {r:?}"); break; }
                Some(_) => {}
                None => break,
            },
            _ = report.tick() => {
                let m = bot.movement().expect("physics enabled");
                let pos = m.position().map_or("not started".into(), |p| format!("({:.3}, {:.3}, {:.3})", p[0], p[1], p[2]));
                let chunks = bot.world().and_then(|w| w.view()).map_or(0, |v| v.len());
                let below = below_feet(&bot);
                println!("phase {phase} pos {pos} | below {below} | chunks {chunks} | corrections {} teleports {}", m.corrections, m.teleports);
            }
            _ = tokio::time::sleep_until(phase_end), if phase < PLAN.len() => {
                phase += 1;
                if phase == PLAN.len() { bot.client().close(); continue; }
                let (secs, forward, strafe, jump, yaw) = PLAN[phase];
                let c = bot.controls().expect("physics enabled");
                (c.forward, c.strafe, c.jump, c.yaw) = (forward, strafe, jump, yaw);
                phase_end += Duration::from_secs(secs);
            }
        }
    }
    Ok(())
}

/// Name of the block under the simulated feet, for diagnosing terrain decoding.
fn below_feet(bot: &Bot) -> String {
    use acacia_world::BlockAccess;
    let (Some(p), Some(w)) = (bot.movement().and_then(|m| m.position()), bot.world()) else { return "-".into() };
    let (Some(view), Some(reg)) = (w.view(), w.registry()) else { return "-".into() };
    let (x, y, z) = (p[0].floor() as i32, (p[1] - 0.01).floor() as i32, p[2].floor() as i32);
    let id = view.block(x, y, z);
    let top = (-64..320).rev().map(|y| (y, view.block(x, y, z))).find(|&(_, b)| b != reg.air_id());
    let top = top.map_or("none".into(), |(ty, b)| format!("{}@{ty}", reg.get(b).map_or("?", |s| s.name)));
    format!("{} @y{y} (column top: {top}, air id {})", reg.get(id).map_or("?", |s| s.name), reg.air_id())
}

fn tracing_subscriber_init() {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
}
