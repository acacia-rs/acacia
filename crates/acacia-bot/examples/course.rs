//! Physics obstacle course on a server where the bot is operator: builds a walled lane along +X
//! (steps, slabs, stairs, a parkour gap, a ladder, a pool) and walks it leg by leg, reporting the
//! server's movement corrections per leg.
//! `cargo run -p acacia-bot --example course -- <server> <name|@account> [java|bedrock] [legs]`
//! `java` for Geyser (Java block-state syntax); `legs` is a comma list of leg names to run.
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::Client;
use acacia_bot::pathfind::Goal;
use acacia_bot::{Bot, BotConfig};

const LEG_TIMEOUT: Duration = Duration::from_secs(30);

struct Leg {
    name: &'static str,
    /// Goal offset from the course origin (the bot's spawn feet block).
    goal: [i32; 3],
    /// Blocks to set, as (offset, block) with `{stairs_e}`/`{ladder_w}` placeholders.
    build: &'static [([i32; 3], [i32; 3], &'static str)],
}

const LEGS: &[Leg] = &[
    Leg { name: "flat", goal: [3, 0, 0], build: &[] },
    Leg { name: "step", goal: [8, 0, 0], build: &[([5, 0, -1], [5, 0, 1], "stone")] },
    Leg {
        name: "stair2",
        goal: [14, 0, 0],
        build: &[([10, 0, -1], [10, 0, 1], "stone"), ([11, 0, -1], [12, 1, 1], "stone")],
    },
    Leg {
        name: "slabs",
        goal: [20, 0, 0],
        build: &[
            ([15, 0, -1], [15, 0, 1], "{slab}"),
            ([16, 0, -1], [17, 0, 1], "stone"),
            ([17, 1, -1], [17, 1, 1], "{slab}"),
        ],
    },
    Leg {
        name: "stairs",
        goal: [26, 0, 0],
        build: &[
            ([21, 0, -1], [21, 0, 1], "{stairs_e}"),
            ([22, 0, -1], [23, 0, 1], "stone"),
            ([22, 1, -1], [22, 1, 1], "{stairs_e}"),
            ([23, 1, -1], [23, 1, 1], "stone"),
        ],
    },
    Leg { name: "gap", goal: [32, 0, 0], build: &[([28, -3, -1], [29, -1, 1], "air")] },
    Leg {
        name: "ladder",
        goal: [38, 0, 0],
        build: &[
            ([34, 0, -1], [34, 3, 1], "stone"),
            ([35, 0, -1], [35, 1, 1], "stone"),
            ([33, 0, 0], [33, 3, 0], "{ladder_w}"),
        ],
    },
    Leg {
        name: "pool",
        goal: [47, 0, 0],
        build: &[([40, -2, -1], [44, -1, 1], "water")],
    },
    Leg { name: "float", goal: [42, -1, 0], build: &[] },
    Leg { name: "exit", goal: [47, 0, 0], build: &[] },
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "@default".into());
    let java = args.next().is_some_and(|d| d == "java");
    let only: Option<Vec<String>> = args.next().map(|l| l.split(',').map(str::to_owned).collect());

    let mut builder = Client::builder(&server);
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(&name),
    };
    let mut bot = Bot::connect(builder, BotConfig { physics: true, ..BotConfig::default() }).await?;
    while bot.movement().is_none_or(|m| !m.is_started()) {
        bot.wait_ticks(1).await?;
    }
    bot.wait_ticks(20).await?;
    if only.as_ref().is_none_or(|o| o.iter().any(|n| n == "respawn")) {
        respawn_check(&mut bot).await?;
    }
    let [x, y, z] = bot.block_position().expect("movement started");
    println!("course origin [{x}, {y}, {z}] ({})", if java { "java" } else { "bedrock" });
    build(&mut bot, [x, y, z], java).await?;

    let (mut total_corr, mut failed) = (0, 0);
    for leg in LEGS {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == leg.name)) {
            continue;
        }
        let before = counts(&bot);
        let t = Instant::now();
        let goal = Goal::Block([x + leg.goal[0], y + leg.goal[1], z + leg.goal[2]]);
        let result = tokio::time::timeout(LEG_TIMEOUT, bot.goto(goal)).await;
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
        println!("{:8} {outcome:<8} {:5.1}s  corrections {corr}  teleports {tp}", leg.name, t.elapsed().as_secs_f32());
    }
    println!("total corrections {total_corr}, failed legs {failed}");
    bot.disconnect().await;
    Ok(())
}

/// Dies (`/kill`), expects auto-respawn, then idles: movement must resume without corrections.
async fn respawn_check(bot: &mut Bot) -> Result<(), Box<dyn std::error::Error>> {
    let before = counts(bot);
    let t = Instant::now();
    bot.client().command("/kill @s");
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
    bot.movement().map_or((0, 0), |m| (m.corrections, m.teleports))
}

/// Clears a walled lane from the origin and places every leg's obstacles.
async fn build(bot: &mut Bot, [x, y, z]: [i32; 3], java: bool) -> Result<(), Box<dyn std::error::Error>> {
    let fill = |a: [i32; 3], b: [i32; 3], block: &str| {
        format!("/fill {} {} {} {} {} {} {block}", x + a[0], y + a[1], z + a[2], x + b[0], y + b[1], z + b[2])
    };
    let mut cmds = vec![
        fill([-2, 0, -3], [50, 8, 3], "air"),
        fill([-2, -4, -3], [50, -1, 3], "stone"),
        fill([-2, 0, -2], [50, 5, -2], "glass"),
        fill([-2, 0, 2], [50, 5, 2], "glass"),
        fill([-2, 0, -2], [-2, 5, 2], "glass"),
    ];
    for leg in LEGS {
        for &(a, b, block) in leg.build {
            cmds.push(fill(a, b, &resolve(block, java)));
        }
    }
    for cmd in cmds {
        bot.client().command(&cmd);
        bot.wait_ticks(2).await?;
    }
    bot.wait_ticks(40).await?;
    Ok(())
}

fn resolve(block: &str, java: bool) -> String {
    match (block, java) {
        ("{slab}", _) => "smooth_stone_slab".into(),
        ("{stairs_e}", true) => "oak_stairs[facing=east]".into(),
        ("{stairs_e}", false) => "oak_stairs [\"weirdo_direction\"=0]".into(),
        ("{ladder_w}", true) => "ladder[facing=west]".into(),
        ("{ladder_w}", false) => "ladder [\"facing_direction\"=4]".into(),
        (b, _) => b.into(),
    }
}
