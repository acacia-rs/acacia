//! Live check of the process layer (acacia-pathfind/docs/process-layer.md), PASS/FAIL per scenario:
//! `backfill` tunnels through a stone block with Backfill on, then every cell of the block must be solid again;
//! `follow` sends bot A on a goto, teleports bot B (the followed player) beside its route, later disconnects B:
//! A must turn to B, then finish its goto. Needs the test pack off (`tools/testbox-bds.sh unpack`).
//! `cargo run -p acacia-bot --example processes -- <server> <nameA> <nameB>`; `ONLY=backfill|follow`.
mod support;

use acacia_bot::client::Client;
use acacia_bot::pathfind::process::Status;
use acacia_bot::pathfind::{Backfill, Follow, Goal, Goto, GotoOpts, NavStatus, Processes};
use acacia_bot::state::Trackers;
use acacia_bot::{Bot, BotConfig};
use support::{Error, Pad, corrections};

/// B is teleported beside the route this many ticks into the goto, and leaves this many ticks after joining.
const B_ARRIVES: u32 = 30;
const B_LEAVES: u32 = 260;
const MAX_TICKS: u32 = 2400;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tokio::task::LocalSet::new().run_until(run()).await
}

async fn run() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19170".into());
    let name_a = args.next().unwrap_or_else(|| "ProcA".into());
    let name_b = args.next().unwrap_or_else(|| "ProcB".into());
    let only = std::env::var("ONLY").ok();
    let config = BotConfig { trackers: Trackers { entities: true, ..Trackers::default() }, ..BotConfig::default() };
    let (mut a, pad) = support::connect_config(&server, &name_a, config).await?;
    let mut failed = 0;
    for name in ["backfill", "follow"] {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let result = match name {
            "backfill" => backfill(&mut a, &pad).await,
            _ => follow(&mut a, &pad, &server, &name_b).await,
        };
        match result {
            Ok(summary) => println!("PASS {name}: {summary}"),
            Err(e) => {
                failed += 1;
                println!("FAIL {name}: {e}");
            }
        }
    }
    println!("{failed} failed");
    pad.go_home(&mut a).await?;
    a.disconnect().await;
    Ok(())
}

async fn commands(a: &mut Bot, pad: &Pad, cmds: &[&str]) -> Result<(), Error> {
    for c in cmds {
        pad.run(a, c);
        a.wait_ticks(2).await?;
    }
    Ok(())
}

async fn backfill(a: &mut Bot, pad: &Pad) -> Result<String, Error> {
    // Across the pad and too high to climb: tunnels.
    let (lo, hi) = ([2, 0, support::PAD_MIN[2]], [9, 9, support::PAD_MAX[2]]);
    pad.build(a, &[(lo, hi, "stone")], 5).await?;
    commands(a, pad, &["/clear @s", "/give @s diamond_pickaxe", "/give @s cobblestone 64"]).await?;
    pad.teleport(a, [0.5, 0.0, 0.5], -90.0).await?;
    let mut opts = GotoOpts::default();
    opts.search.allow_dig = true;
    let mut procs = Processes::new();
    let goto = procs.add(Goto::with(Goal::Block(pad.block([12, 0, 0])), opts), 10);
    procs.add(Backfill::new(), 20);
    let open = |a: &Bot| {
        let cells = (lo[0]..=hi[0]).flat_map(|x| (lo[1]..=hi[1]).flat_map(move |y| (lo[2]..=hi[2]).map(move |z| [x, y, z])));
        cells.map(|c| pad.block(c)).filter(|p| a.block_name(*p).is_none_or(|n| n == "minecraft:air")).collect::<Vec<_>>()
    };
    let before = corrections(a);
    let (mut ticks, mut dug) = (0, 0);
    while ticks < MAX_TICKS && !(procs.outcome(goto).is_some() && open(a).is_empty()) {
        dug += u32::from(procs.tick(a).await? == Status::Moving(NavStatus::Working));
        ticks += 1;
    }
    if dug == 0 {
        return Err("never dug: the goto went round the block".into());
    }
    match procs.outcome(goto) {
        Some(Ok(())) => {}
        other => return Err(format!("goto: {other:?} after {ticks} ticks at {:?}", a.block_position()).into()),
    }
    let left = open(a);
    if !left.is_empty() {
        return Err(format!("{} cells left open: {left:?}", left.len()).into());
    }
    Ok(format!("{ticks} ticks, {dug} moves dug, {} corrections, every dug cell filled", corrections(a) - before))
}

async fn follow(a: &mut Bot, pad: &Pad, server: &str, name_b: &str) -> Result<String, Error> {
    pad.build(a, &[], 5).await?;
    commands(a, pad, &["/clear @s"]).await?;
    pad.teleport(a, [0.5, 0.0, 0.5], -90.0).await?;
    let mut b = Bot::connect(Client::builder(server).offline(name_b), BotConfig::default()).await?;
    let keeper = tokio::task::spawn_local(async move {
        let _ = b.wait_ticks(B_LEAVES).await;
        b.disconnect().await;
    });
    let mut procs = Processes::new();
    let goto = procs.add(Goto::new(Goal::Block(pad.block([20, 0, 0]))), 10);
    let follow = procs.add(Follow::player(name_b, 3.0), 50);
    let spot = pad.block([6, 0, 10]);
    let before = corrections(a);
    let (mut ticks, mut followed, mut closest) = (0, 0, f32::INFINITY);
    while ticks < MAX_TICKS && procs.outcome(goto).is_none() {
        if ticks == B_ARRIVES {
            pad.run(a, &format!("/tp {name_b} {} {} {}", spot[0] as f32 + 0.5, spot[1], spot[2] as f32 + 0.5));
        }
        procs.tick(a).await?;
        ticks += 1;
        followed += u32::from(procs.arbiter().active() == Some(follow));
        let me = a.bedrock().and_then(|b| b.movement()).and_then(|m| m.position());
        let them = a.state().entities.players().find(|e| e.username.as_deref() == Some(name_b)).map(|e| e.feet());
        if let (Some(me), Some(f)) = (me, them) {
            closest = closest.min((me[0] - f.x).hypot(me[2] - f.z).hypot(me[1] - f.y));
        }
    }
    keeper.abort();
    match procs.outcome(goto) {
        Some(Ok(())) => {}
        other => return Err(format!("goto: {other:?} after {ticks} ticks at {:?}", a.block_position()).into()),
    }
    if followed == 0 || closest > 4.5 {
        return Err(format!("never followed B: {followed} ticks in control, closest {closest:.2}").into());
    }
    Ok(format!("{ticks} ticks, follow in control {followed} ticks, closest {closest:.2}, {} corrections", corrections(a) - before))
}
