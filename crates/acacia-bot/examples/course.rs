//! Physics obstacle course on a server where the bot is operator: builds a walled lane along +X
//! (steps, slabs, stairs, a gap, a ladder, a pool, then parkour legs: gaps of 3 and 4, a leap up 1 and
//! down 2, a diagonal leap, stepping stones and a 40-block sprint) and walks it leg by leg, reporting the
//! server's movement corrections and the ticks per leg (`course/legs.rs`).
//! `cargo run -p acacia-bot --example course -- <server> <name|@account> [java|bedrock] [legs] [flags]`
//! `java` for Geyser (Java block-state syntax); `legs` is a comma list of leg names to run.
//! Flags: `--profile optimal|human`, `--macros off`, `--shortcuts off`, `--seed <n>` (aim jitter), `--lift`
//! (levitate the bot in mid-air over the gap3 leap: a live macro divergence). `BEDROCK_RECORD=<file>` records a trace.
#[path = "course/legs.rs"]
mod legs;

use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::Client;
use acacia_bot::pathfind::{Goal, GotoOpts, MovementProfile, NavStatus, Navigator};
use acacia_bot::{ActionError, Bot, BotConfig};
use legs::{LEGS, LIFT, LIFT_LEG, LIFT_X, lane, resolve};

const LEG_TIMEOUT: Duration = Duration::from_secs(30);

type Error = Box<dyn std::error::Error>;

struct Args {
    server: String,
    name: String,
    java: bool,
    only: Option<Vec<String>>,
    opts: GotoOpts,
    lift: bool,
}

fn parse() -> Args {
    let (mut flags, mut pos) = (Vec::new(), Vec::new());
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lift" => flags.push((a, String::new())),
            f if f.starts_with("--") => flags.push((a.clone(), it.next().unwrap_or_default())),
            _ => pos.push(a),
        }
    }
    let mut opts = GotoOpts::default();
    let mut lift = false;
    for (flag, value) in flags {
        match (flag.as_str(), value.as_str()) {
            ("--profile", "human") => opts.search.profile = MovementProfile::Human,
            ("--profile", "optimal") => opts.search.profile = MovementProfile::Optimal,
            ("--macros", "off") => opts.search.macros = false,
            ("--shortcuts", "off") => opts.shortcuts = false,
            ("--seed", n) => opts.seed = Some(n.parse().expect("--seed <n>")),
            ("--lift", _) => lift = true,
            _ => panic!("unknown flag {flag} {value}"),
        }
    }
    let mut pos = pos.into_iter();
    Args {
        server: pos.next().unwrap_or_else(|| "127.0.0.1:19140".into()),
        name: pos.next().unwrap_or_else(|| "@default".into()),
        java: pos.next().is_some_and(|d| d == "java"),
        only: pos.next().map(|l| l.split(',').map(str::to_owned).collect()),
        opts,
        lift,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let args = parse();
    let mut builder = Client::builder(&args.server);
    builder = match args.name.strip_prefix('@') {
        Some(account) => {
            let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(&args.name),
    };
    let record = std::env::var_os("BEDROCK_RECORD").map(Into::into);
    let mut bot = Bot::connect(builder, BotConfig { physics: true, record, ..BotConfig::default() }).await?;
    while bot.bedrock().and_then(|b| b.movement()).is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    bot.wait_ticks(20).await?;
    let wanted = |name: &str| args.only.as_ref().is_none_or(|o| o.iter().any(|n| n == name));
    if wanted("respawn") {
        respawn_check(&mut bot).await?;
    }
    let origin = bot.block_position().expect("movement started");
    let s = &args.opts.search;
    println!("course origin {origin:?} ({}), profile {:?}, macros {}, seed {:?}", if args.java { "java" } else { "bedrock" }, s.profile, s.macros, args.opts.seed);
    build(&mut bot, origin, args.java, false).await?;

    let (mut total_corr, mut failed, mut far_built) = (0, 0, false);
    for leg in LEGS.iter().filter(|l| wanted(l.name)) {
        if leg.far() && !far_built {
            build(&mut bot, origin, args.java, true).await?;
            far_built = true;
        }
        let [x, y, z] = origin;
        let lift = (args.lift && leg.name == LIFT_LEG).then_some(x as f32);
        let before = counts(&bot);
        let t = Instant::now();
        let goal = Goal::Block([x + leg.goal[0], y + leg.goal[1], z + leg.goal[2]]);
        let mut nav = Navigator::new(goal, args.opts.clone());
        let mut ticks = 0;
        let result = tokio::time::timeout(LEG_TIMEOUT, run_leg(&mut bot, &mut nav, &mut ticks, lift)).await;
        bot.stop_pathing();
        bot.wait_ticks(10).await?;
        let after = counts(&bot);
        let (corr, tp) = (after.0 - before.0, after.1 - before.1);
        total_corr += corr;
        let outcome = match result {
            Ok(Ok(())) => "ok".to_string(),
            Ok(Err(e)) => format!("FAILED ({e})"),
            Err(_) => "TIMEOUT".into(),
        };
        failed += u32::from(outcome != "ok");
        println!(
            "{:8} {outcome:<8} {ticks:4} ticks {:5.1}s  corrections {corr}  teleports {tp}  diverged {}  replans {}",
            leg.name,
            t.elapsed().as_secs_f32(),
            nav.divergences(),
            nav.replans()
        );
    }
    println!("total corrections {total_corr}, failed legs {failed}");
    // Offline players rejoin where they left: back to the origin, so the next run builds in the same place.
    bot.command(&format!("/tp @s {} {} {}", origin[0] as f32 + 0.5, origin[1], origin[2] as f32 + 0.5));
    bot.wait_ticks(10).await?;
    bot.disconnect().await;
    Ok(())
}

/// Navigates until arrival, counting ticks. `lift`: the origin's x, to levitate the bot once it is in the
/// air over the lift leg's gap.
async fn run_leg(bot: &mut Bot, nav: &mut Navigator, ticks: &mut u32, lift: Option<f32>) -> Result<(), ActionError> {
    let mut lifted = false;
    loop {
        if let (Some(x0), false) = (lift, lifted)
            && let Some(m) = bot.bedrock().and_then(|b| b.movement())
            && let Some(p) = m.position()
            && !m.on_ground()
            && (LIFT_X.0..LIFT_X.1).contains(&(p[0] - x0))
        {
            bot.command(LIFT);
            println!("lift at tick {ticks}, x offset {:.2}", p[0] - x0);
            lifted = true;
        }
        if nav.tick(bot).await? == NavStatus::Arrived {
            return Ok(());
        }
        *ticks += 1;
    }
}

/// Dies (`/kill`), expects auto-respawn, then idles: movement must resume without corrections.
async fn respawn_check(bot: &mut Bot) -> Result<(), Error> {
    let before = counts(bot);
    let t = Instant::now();
    bot.command("/kill @s");
    let deaths = bot.state().player.deaths;
    for _ in 0..200 {
        bot.wait_ticks(1).await?;
        if bot.state().player.deaths > deaths && bot.state().player.alive {
            break;
        }
    }
    let died = bot.state().player.deaths > deaths;
    bot.wait_ticks(40).await?;
    let after = counts(bot);
    let p = &bot.state().player.position;
    let outcome = if !died { "NO DEATH" } else if bot.state().player.alive { "ok" } else { "STILL DEAD" };
    println!(
        "{:8} {outcome:<8} {:5.1}s  corrections {}  teleports {}  at ({:.1}, {:.1}, {:.1})",
        "respawn",
        t.elapsed().as_secs_f32(),
        after.0 - before.0,
        after.1 - before.1,
        p.x,
        p.y,
        p.z
    );
    Ok(())
}

fn counts(bot: &Bot) -> (u32, u32) {
    bot.bedrock().and_then(|b| b.movement()).map_or((0, 0), |m| (m.corrections, m.teleports))
}

/// Clears one section of the walled lane from the origin and places its legs' obstacles; no mobs (their
/// hits are corrections).
async fn build(bot: &mut Bot, [x, y, z]: [i32; 3], java: bool, far: bool) -> Result<(), Error> {
    let fill = |a: [i32; 3], b: [i32; 3], block: &str| {
        format!("/fill {} {} {} {} {} {} {block}", x + a[0], y + a[1], z + a[2], x + b[0], y + b[1], z + b[2])
    };
    let mut cmds = vec!["/difficulty peaceful".to_string(), "/kill @e[type=!player]".into()];
    cmds.extend(lane(far).into_iter().map(|(a, b, block)| fill(a, b, block)));
    for leg in LEGS.iter().filter(|l| l.far() == far) {
        cmds.extend(leg.build.iter().map(|&(a, b, block)| fill(a, b, &resolve(block, java))));
    }
    for cmd in cmds {
        bot.command(&cmd);
        bot.wait_ticks(2).await?;
    }
    bot.wait_ticks(40).await?;
    Ok(())
}
