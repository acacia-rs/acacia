//! Live check of digging, bridging, pillaring and gates: builds each course in the sky with
//! commands (needs operator), then runs the navigator through it and prints PASS/FAIL.
//! `cargo run -p acacia-bot --example dig_course -- <server> <name|@account> [x z]`
//! `ONLY=tunnel,bridge` picks courses; the courses are built at y 150 around x/z (default 1000).
use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::Client;
use acacia_bot::pathfind::{Goal, GotoOpts, NavStatus, Navigator};
use acacia_bot::{Bot, BotConfig};

const Y: i32 = 150;
const TIMEOUT: Duration = Duration::from_secs(90);

struct Course {
    name: &'static str,
    /// Commands with `{x}`/`{y}`/`{z}` replaced by the course origin (feet level).
    build: &'static [&'static str],
    /// Goal relative to the origin.
    goal: [i32; 3],
}

const COURSES: &[Course] = &[
    Course {
        name: "tunnel",
        build: &[
            "fill {x-1} {y-1} {z-1} {x+13} {y+2} {z+1} stone",
            "fill {x} {y} {z} {x+3} {y+1} {z} air",
            "fill {x+9} {y} {z} {x+12} {y+1} {z} air",
        ],
        goal: [12, 0, 0],
    },
    Course {
        name: "bridge",
        build: &["fill {x} {y-1} {z} {x+2} {y-1} {z} stone", "fill {x+7} {y-1} {z} {x+10} {y-1} {z} stone"],
        goal: [9, 0, 0],
    },
    Course { name: "pillar", build: &["fill {x-1} {y-1} {z-1} {x+1} {y-1} {z+1} stone"], goal: [0, 3, 0] },
    Course {
        name: "gate",
        build: &[
            "fill {x-1} {y-1} {z-1} {x+13} {y+2} {z+1} stone",
            "fill {x} {y} {z} {x+12} {y+1} {z} air",
            "setblock {x+5} {y} {z} fence_gate [\"direction\"=1]",
        ],
        goal: [12, 0, 0],
    },
    Course { name: "down", build: &["fill {x-1} {y-5} {z-1} {x+1} {y-1} {z+1} stone"], goal: [0, -3, 0] },
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server = args.first().cloned().unwrap_or_else(|| "127.0.0.1:19190".into());
    let name = args.get(1).cloned().unwrap_or_else(|| "Digger".into());
    let base_x: i32 = args.get(2).map_or(Ok(1000), |s| s.parse())?;
    let base_z: i32 = args.get(3).map_or(Ok(1000), |s| s.parse())?;
    let only: Option<Vec<String>> = std::env::var("ONLY").ok().map(|s| s.split(',').map(str::to_owned).collect());

    let mut builder = Client::builder(&server);
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let auth = Arc::new(AuthClient::new(AuthConfig::default())?);
            let account = Account::new(auth, Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(&name),
    };
    let mut bot = Bot::connect(builder, BotConfig { physics: true, ..BotConfig::default() }).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    println!("spawned as {}", bot.client().display_name());

    let mut failed = 0;
    for (i, course) in COURSES.iter().enumerate() {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == course.name)) {
            continue;
        }
        let origin = [base_x, Y, base_z + 10 * i as i32];
        match run(&mut bot, course, origin).await {
            Ok(summary) => println!("PASS {}: {summary}", course.name),
            Err(e) => {
                failed += 1;
                println!("FAIL {}: {e}", course.name);
            }
        }
    }
    println!("{failed} failed");
    bot.disconnect().await;
    Ok(())
}

async fn run(bot: &mut Bot, course: &Course, [x, y, z]: [i32; 3]) -> Result<String, Box<dyn std::error::Error>> {
    let cmd = |bot: &Bot, c: &str| bot.client().command(&format!("/{}", expand(c, x, y, z)));
    // Park above the course so the chunks load, then clear it and build.
    cmd(bot, "tp @s {x} {y+20} {z}");
    bot.wait_ticks(60).await?;
    cmd(bot, "fill {x-3} {y-6} {z-3} {x+15} {y+6} {z+3} air");
    for c in course.build {
        cmd(bot, c);
    }
    for c in ["clear @s", "give @s diamond_pickaxe", "give @s dirt 64", "effect @s clear"] {
        cmd(bot, c);
    }
    cmd(bot, "tp @s {x} {y} {z}");
    bot.wait_ticks(60).await?;

    let start = bot.block_position().ok_or("no position")?;
    let [dx, dy, dz] = course.goal;
    let goal = Goal::Block([x + dx, y + dy, z + dz]);
    let mut opts = GotoOpts::default();
    (opts.search.allow_dig, opts.search.allow_bridge) = (true, true);
    let mut nav = Navigator::new(goal.clone(), opts);
    let corrections = bot.movement().map_or(0, |m| m.corrections);
    let started = tokio::time::Instant::now();
    let mut worked = 0u32;
    loop {
        if started.elapsed() > TIMEOUT {
            bot.stop_pathing();
            return Err(format!("timed out at {:?} (from {start:?} to {goal:?})", bot.block_position()).into());
        }
        match nav.tick(bot).await {
            Ok(NavStatus::Arrived) => break,
            Ok(NavStatus::Working) => worked += 1,
            Ok(_) => {}
            Err(e) => return Err(format!("{e} at {:?} after {} replans", bot.block_position(), nav.replans()).into()),
        }
    }
    let corrections = bot.movement().map_or(0, |m| m.corrections) - corrections;
    Ok(format!(
        "{:.1}s, {worked} working ticks, {} replans, {corrections} corrections, at {:?}",
        started.elapsed().as_secs_f32(),
        nav.replans(),
        bot.block_position()
    ))
}

/// Replaces `{x}`, `{y+2}`, `{z-1}`, ... with absolute coordinates.
fn expand(template: &str, x: i32, y: i32, z: i32) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = open + rest[open..].find('}').expect("unclosed {");
        let expr = &rest[open + 1..close];
        let base = match &expr[..1] {
            "x" => x,
            "y" => y,
            _ => z,
        };
        out.push_str(&(base + expr[1..].parse::<i32>().unwrap_or(0)).to_string());
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}
