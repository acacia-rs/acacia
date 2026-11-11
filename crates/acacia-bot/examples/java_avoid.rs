//! Java bot: walks to `<x> <y> <z>` with hazard tracking on and prints how close it came (horizontally) to the
//! nearest mob and how far it swerved in x. `ACACIA_BLIND` tracks nothing: the control that walks straight on.
//! `ACACIA_TRACE` prints the bot's eye and the nearest mob every tick.
//! `cargo run -p acacia-bot --features java --example java_avoid -- <host> <port> <x> <y> <z> [name]`

use acacia_bot::pathfind::{Goal, GotoOpts, NavStatus, Navigator};
use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, x, y, z] = &args[..5] else { return Err("usage: java_avoid <host> <port> <x> <y> <z> [name]".into()) };
    let goal = [x.parse()?, y.parse()?, z.parse()?];
    let name = args.get(5).map_or("Avoider", String::as_str);

    let mut config = BotConfig::default();
    config.trackers.hazards = std::env::var_os("ACACIA_BLIND").is_none();
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, config).await?;
    bot.wait_ticks(60).await?;
    let mobs: Vec<String> = bot.state().entities.iter().filter(|e| !e.is_player()).map(|e| e.kind.clone()).collect();
    println!("tracking {mobs:?}");
    let mut nav = Navigator::new(Goal::Block(goal), GotoOpts::default());
    let (mut closest, mut swerve) = (f32::MAX, 0.0_f32);
    let start_x = bot.eye_position()[0];
    let trace = std::env::var_os("ACACIA_TRACE").is_some();
    let result = loop {
        let eye = bot.eye_position();
        let apart = |e: &acacia_bot::state::Entity| {
            let f = e.feet();
            (f.x - eye[0]).hypot(f.z - eye[2])
        };
        let near = bot.state().entities.iter().filter(|e| !e.is_player()).map(apart).fold(f32::MAX, f32::min);
        if trace && let Some(m) = bot.state().entities.iter().filter(|e| !e.is_player()).min_by(|a, b| apart(a).total_cmp(&apart(b))) {
            let (f, v) = (m.feet(), &m.motion);
            println!("trace eye {eye:.2?} mob {:.2?} motion {:.3?}", [f.x, f.y, f.z], [v.x, v.y, v.z]);
        }
        closest = closest.min(near);
        swerve = swerve.max((eye[0] - start_x).abs());
        match nav.tick(&mut bot).await {
            Ok(NavStatus::Arrived) => break Ok(()),
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };
    let left = bot.state().entities.iter().filter(|e| !e.is_player()).count();
    println!("goto {result:?} at {:?}; closest mob {closest:.2}, swerve {swerve:.2} blocks, {left} mobs left", bot.block_position());
    bot.disconnect().await;
    Ok(())
}
