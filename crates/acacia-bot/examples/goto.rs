//! Physics bot: waits for movement to start, then walks to a target relative to its spawn,
//! printing progress every second.
//! `cargo run -p acacia-bot --example goto -- <server> <name|@account> <dx> <dz> [dy]`
//! With `dy` the goal is that exact block; without it any y at that x/z.
//! `BEDROCK_PROXY=host:port:user:pass` as in the other examples; `BEDROCK_DIG` / `BEDROCK_BRIDGE`
//! let the path dig and place scaffold blocks.
use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::{Client, Socks5Proxy};
use acacia_bot::pathfind::{Goal, GotoOpts, NavStatus, Navigator};
use acacia_bot::proto::types::Vec3f;
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig};
use acacia_world::BlockAccess;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Walker".into());
    let offset: Vec<i32> = args.iter().skip(2).map(|a| a.parse()).collect::<Result<_, _>>()?;
    let (dx, dz, dy) = match offset[..] {
        [dx, dz] => (dx, dz, None),
        [dx, dz, dy] => (dx, dz, Some(dy)),
        [] => (10, 0, None),
        _ => return Err("expected <dx> <dz> [dy]".into()),
    };
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
    let config = BotConfig { physics: true, trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let mut bot = Bot::connect(builder, config).await?;
    println!("spawned as {}", bot.client().display_name());

    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    // BEDROCK_TP="x y z": teleport there first (needs operator), so routes start somewhere repeatable.
    if let Ok(tp) = std::env::var("BEDROCK_TP") {
        bot.client().command(&format!("/tp @s {tp}"));
        bot.wait_ticks(40).await?;
    }
    if std::env::var("BEDROCK_DUMP").is_ok() {
        print_surroundings(&bot);
    }
    // Let the surrounding chunks arrive before planning.
    bot.wait_ticks(40).await?;
    let [x, y, z] = bot.block_position().expect("movement started");
    let goal = match dy {
        Some(dy) => Goal::Block([x + dx, y + dy, z + dz]),
        None => Goal::XZ(x + dx, z + dz),
    };
    println!("at [{x}, {y}, {z}], going to {goal:?}");

    let mut opts = GotoOpts::default();
    opts.search.allow_dig = std::env::var("BEDROCK_DIG").is_ok();
    opts.search.allow_bridge = std::env::var("BEDROCK_BRIDGE").is_ok();
    let mut nav = Navigator::new(goal, opts);
    let mut report = tokio::time::Instant::now();
    let started = report;
    let mut seen = 0;
    let result = loop {
        let status = nav.tick(&mut bot).await;
        let corrections = bot.movement().map_or(0, |m| m.corrections);
        if corrections > seen {
            seen = corrections;
            print_surroundings(&bot);
        }
        match status {
            Ok(NavStatus::Arrived) => break Ok(()),
            Ok(status) if report.elapsed() >= Duration::from_secs(1) => {
                report = tokio::time::Instant::now();
                print_progress(&bot, &nav, status);
            }
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    print_progress(&bot, &nav, NavStatus::Arrived);
    match result {
        Ok(()) => println!("arrived in {:.1}s", started.elapsed().as_secs_f32()),
        Err(e) => println!("failed: {e}"),
    }
    bot.wait_ticks(20).await?;
    if bot.movement().is_some_and(|m| m.corrections > seen) {
        print_surroundings(&bot);
    }
    bot.disconnect().await;
    Ok(())
}

/// On a correction: nearby entities (pushing) and a 5×3×5 grid of the blocks around the feet:
/// `.` air, `#` full cube, `0`-`7` water decay (`F` falling), `~` water in the liquid layer only,
/// `+` waterlogged block, letters = other blocks.
fn print_surroundings(bot: &Bot) {
    let Some(feet) = bot.movement().and_then(|m| m.position()) else { return };
    let at = Vec3f { x: feet[0], y: feet[1], z: feet[2] };
    let near: Vec<_> = bot.state().entities.iter().filter(|e| e.distance_sq(&at) < 9.0).map(|e| e.kind.as_str()).collect();
    println!("CORRECTION at ({:.3}, {:.3}, {:.3}) entities {near:?}", feet[0], feet[1], feet[2]);
    let Some((w, r)) = bot.world().and_then(|w| Some((w.view()?, w.registry()?))) else { return };
    let [x, y, z] = [feet[0].floor() as i32, feet[1].floor() as i32, feet[2].floor() as i32];
    let mut legend: Vec<&str> = Vec::new();
    for dy in (-1..=1).rev() {
        let rows: Vec<String> = (-2..=2)
            .map(|dz| {
                (-2..=2)
                    .map(|dx| {
                        let (bx, by, bz) = (x + dx, y + dy, z + dz);
                        let Some(s) = r.get(w.block(bx, by, bz)) else { return '?' };
                        let waterlogged = r.get(w.liquid(bx, by, bz)).is_some_and(|l| l.is_water());
                        if s.is_air() {
                            if waterlogged { '~' } else { '.' }
                        } else if s.is_water() {
                            if s.liquid_depth >= 8 { 'F' } else { char::from(b'0' + s.liquid_depth) }
                        } else if waterlogged {
                            '+'
                        } else if s.is_full_cube() {
                            '#'
                        } else {
                            let i = legend.iter().position(|n| *n == s.name).unwrap_or_else(|| {
                                legend.push(s.name);
                                legend.len() - 1
                            });
                            char::from(b'a' + i as u8)
                        }
                    })
                    .collect()
            })
            .collect();
        println!("  dy {dy:+}: {}", rows.join(" | "));
    }
    if !legend.is_empty() {
        println!("  legend: {}", legend.iter().enumerate().map(|(i, n)| format!("{}={n}", char::from(b'a' + i as u8))).collect::<Vec<_>>().join(" "));
    }
}

fn print_progress(bot: &Bot, nav: &Navigator, status: NavStatus) {
    let m = bot.movement().expect("physics enabled");
    let pos = m.position().map_or("-".into(), |p| format!("({:.2}, {:.2}, {:.2})", p[0], p[1], p[2]));
    let next = nav.follower().and_then(|f| f.ahead().first().map(|n| (n.pos, n.kind)));
    println!(
        "{status:?} pos {pos} | remaining {}{} next {next:?} | replans {} | corrections {} teleports {}",
        nav.remaining(),
        if nav.is_partial() { " (partial)" } else { "" },
        nav.replans(),
        m.corrections,
        m.teleports,
    );
}
