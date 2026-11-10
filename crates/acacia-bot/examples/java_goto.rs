//! Java bot: joins an offline-mode server, waits 3 s, then walks to a block with the shared pathfinder, digging
//! through what is in the way when `ACACIA_DIG` is set and bridging with held blocks when `ACACIA_BRIDGE` is.
//! `cargo run -p acacia-bot --features java --example java_goto -- <host> <port> <x> <y> <z> [name]`

use acacia_bot::pathfind::{Goal, GotoOpts};
use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port, x, y, z] = &args[..5] else { return Err("usage: java_goto <host> <port> <x> <y> <z> [name]".into()) };
    let goal = [x.parse()?, y.parse()?, z.parse()?];
    let name = args.get(5).map_or("Walker", String::as_str);

    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    println!("start {:?}", bot.block_position());
    let started = std::time::Instant::now();
    let mut opts = GotoOpts::default();
    opts.search.allow_dig = std::env::var_os("ACACIA_DIG").is_some();
    opts.search.allow_bridge = std::env::var_os("ACACIA_BRIDGE").is_some();
    let result = bot.goto_with(Goal::Block(goal), opts).await;
    println!("goto {result:?} after {:.1} s, at {:?}", started.elapsed().as_secs_f32(), bot.block_position());
    let inv = &bot.state().inventory;
    let name = |s: &acacia_bot::state::ItemStack| bot.state().items.name(s.network_id).map(|n| format!("{n}x{}", s.count));
    println!("held {:?}, main 9 {:?}", name(inv.held()), name(&inv.main[9]));
    let resent = bot.java().map_or(0, |j| j.client().inventory.resent);
    println!("held enchantments {:?}, name {:?}; slots resent unchanged {resent}", inv.held().enchantments(), inv.held().custom_name);
    bot.disconnect().await;
    Ok(())
}
