//! Movement fuzzer: random terrain on a floating pad and random held controls, against a server
//! where the bot is operator. Run against BDS in strict mode (docs/DESIGN.md "Checking physics") with
//! `BEDROCK_RECORD=<file>`, then list the disagreeing ticks with the `replay` example.
//! `cargo run -p acacia-bot --example fuzz -- <server> <name|@account> [rounds] [seed] [ticks]`
//! `FUZZ_OFF=pitch,web` disables features (see [`on`]); `FUZZ_BOTS=4` runs four bots at once;
//! `FUZZ_CMD="/effect @s speed 9999 1;/gamerule x y"` runs commands once per bot before the first round.
mod support;

use std::path::PathBuf;

use acacia_bot::movement::Controls;
use acacia_bot::world::SharedWorlds;
use acacia_bot::Bot;
use support::{corrections, Commander, Error, Fill, Pad, PAD_MIN};
use tokio::sync::mpsc;

/// Pads of side-by-side bots are this far apart along x (a pad and its parking spot span 33 blocks).
const PAD_SPACING: i32 = 48;
/// Pads float `PAD_HEIGHT` above this, clear of the test world's terrain and below the build limit.
const PAD_ORIGIN: [i32; 3] = [0, 120, 0];
/// Water must finish spreading before a round: a flow still advancing reaches the server's player a
/// few ticks before its block updates reach ours. A busy server spreads it slower than 5 ticks a block.
const SETTLE_TICKS: u32 = 80;

/// Random terrain stays inside this part of the pad (offsets), and so does the bot.
const AREA_MIN: [i32; 2] = [0, -8];
const AREA_MAX: [i32; 2] = [20, 8];
const FLOORS: &[&str] = &["ice", "packed_ice", "blue_ice", "slime", "honey_block", "soul_sand"];
const OBSTACLES: &[&str] = &["stone", "glass", "smooth_stone_slab", "oak_stairs", "oak_fence", "cobblestone_wall", "white_carpet"];
/// Climbables lean on a stone column on their -z side (ladders and vines need one to face).
const CLIMBABLES: &[&str] = &["ladder [\"facing_direction\"=3]", "vine [\"vine_direction_bits\"=4]", "scaffolding", "twisting_vines"];
const INSIDE: &[&str] = &["powder_snow", "sweet_berry_bush [\"growth\"=3]"];
const SHAPES: &[&str] = &["oak_fence_gate", "oak_trapdoor", "iron_bars", "glass_pane"];
const COLUMN_BASES: &[&str] = &["soul_sand", "magma"];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "@default".into());
    let rounds: u32 = args.next().map_or(Ok(10), |a| a.parse())?;
    let seed: u64 = match args.next() {
        Some(a) => a.parse()?,
        None => std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos() as u64,
    };
    let ticks: u32 = args.next().map_or(Ok(300), |a| a.parse())?;
    // Several bots fuzz side by side (offline-mode server): bot i is "fuzz<i>" on its own pad, with its
    // own seed and trace "<BEDROCK_RECORD>-<i>"; `name` stays connected as the operator that runs their
    // commands (see `support::Commander`).
    let bots: usize = std::env::var("FUZZ_BOTS").map_or(Ok(1), |v| v.parse())?;
    let record = std::env::var("BEDROCK_RECORD").ok();
    println!("seed {seed}");
    let mut runs = tokio::task::JoinSet::new();
    // One world for all bots, as a swarm would share it.
    let worlds = SharedWorlds::new();
    let relay = (bots > 1).then(|| {
        let (tx, rx) = mpsc::unbounded_channel();
        let (server, admin) = (server.clone(), name.clone());
        tokio::spawn(async move {
            if let Err(e) = run_commands(&server, &admin, rx).await {
                eprintln!("admin: {e}");
            }
        });
        tx
    });
    for i in 0..bots {
        let relay = relay.clone();
        let name = if bots == 1 { name.clone() } else { format!("fuzz{i}") };
        let record = record.as_ref().map(|r| if bots == 1 { r.into() } else { PathBuf::from(format!("{r}-{i}")) });
        let (server, worlds) = (server.clone(), worlds.clone());
        runs.spawn(async move {
            // RakNet refuses a second login from the same address within about a second.
            let slot = if bots == 1 { 0 } else { i as u64 + 1 };
            tokio::time::sleep(std::time::Duration::from_secs(2 * slot)).await;
            fuzz_bot(&server, &name, record, worlds, relay, i, seed.wrapping_add(i as u64), rounds, ticks).await.map_err(|e| format!("bot {i}: {e}"))
        });
    }
    while let Some(done) = runs.join_next().await {
        done??;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn fuzz_bot(
    server: &str,
    name: &str,
    record: Option<PathBuf>,
    worlds: SharedWorlds,
    relay: Option<mpsc::UnboundedSender<String>>,
    index: usize,
    seed: u64,
    rounds: u32,
    ticks: u32,
) -> Result<(), Error> {
    let (mut bot, pad) = support::connect_with(server, name, record, worlds).await?;
    let mut pad = pad.at(PAD_ORIGIN).shifted(index as i32 * PAD_SPACING);
    if let Some(relay) = relay {
        pad = pad.relayed(Commander::Relay(relay, name.to_owned()));
    }
    // Spread the seed (xorshift needs a nonzero state): `seed | 1` gave bots n and n+1 one stream.
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    pad.run(&bot, "/effect @s fire_resistance 1000000 0 true");
    for command in std::env::var("FUZZ_CMD").iter().flat_map(|c| c.split(';')) {
        pad.run(&bot, command.trim());
    }
    for round in 0..rounds {
        let terrain = terrain(&mut rng);
        bot.trace_mark(&format!("r{round} build"));
        pad.build(&mut bot, &terrain, SETTLE_TICKS).await?;
        pad.teleport(&mut bot, [0.5, 0.0, 0.5], rng.range(-180, 180) as f32).await?;
        bot.trace_mark(&format!("r{round}"));
        let before = corrections(&bot);
        drive(&mut bot, &pad, &mut rng, ticks).await?;
        println!("bot {index} round {round:3} corrections {}", corrections(&bot) - before);
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}

/// Stays connected as the operator account `admin`, running each command sent on `commands` in order
/// until every sender is gone.
async fn run_commands(server: &str, admin: &str, mut commands: mpsc::UnboundedReceiver<String>) -> Result<(), Error> {
    let (mut bot, _) = support::connect_with(server, admin, None, SharedWorlds::new()).await?;
    loop {
        match commands.try_recv() {
            Ok(command) => {
                bot.client().command(&command);
            }
            Err(mpsc::error::TryRecvError::Empty) => bot.wait_ticks(1).await?,
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
    bot.disconnect().await;
    Ok(())
}

/// Random features: slippery or sticky floor patches, obstacles (partial and state-shaped blocks included),
/// cobwebs, climbables, honey walls, powder snow and berry bushes, still and bubbling pools, lava, low
/// ceilings and flowing water. The start cell stays plain stone.
fn terrain(rng: &mut Rng) -> Vec<Fill<'static>> {
    let mut fills = Vec::new();
    for _ in 0..rng.range(10, 17) {
        let (x, z) = (rng.range(AREA_MIN[0], AREA_MAX[0]), rng.range(AREA_MIN[1], AREA_MAX[1]));
        let (w, d) = (rng.range(0, 4), rng.range(0, 4));
        let (a, b) = ([x, 0, z], [(x + w).min(AREA_MAX[0]), 0, (z + d).min(AREA_MAX[1])]);
        let (kind, floor, obstacle, top, depth) = (rng.range(0, 13), *rng.pick(FLOORS), *rng.pick(OBSTACLES), rng.range(0, 2), rng.range(1, 5));
        let (climbable, inside, shape, base) = (*rng.pick(CLIMBABLES), *rng.pick(INSIDE), *rng.pick(SHAPES), *rng.pick(COLUMN_BASES));
        let pool = ([a[0], -depth, a[2]], [(b[0] + 3).min(AREA_MAX[0]), -1, (b[2] + 3).min(AREA_MAX[1])]);
        match kind {
            0 | 1 if on("floors") => fills.push(([a[0], -1, a[2]], [b[0], -1, b[2]], floor)),
            2 if on("obstacles") => fills.push((a, [b[0], top, b[2]], obstacle)),
            3 if on("web") => fills.push((a, a, "web")),
            // Wide and up to 4 deep (the floor is 5), room for a sprint to dive into a swim.
            4 if on("pools") => fills.push((pool.0, pool.1, "water")),
            // A block placed and removed above a source: BDS `/fill` schedules no liquid update.
            5 if on("flow") => fills.extend([(a, a, "water"), ([x, 1, z], [x, 1, z], "stone"), ([x, 1, z], [x, 1, z], "air")]),
            6 if on("climb") => fills.extend([([x, 0, z - 1], [x, 2 + top, z - 1], "stone"), ([x, 0, z], [x, 2 + top, z], climbable)]),
            7 if on("honeywall") => fills.push((a, [b[0], 1 + top, a[2]], "honey_block")),
            8 if on("inside") => fills.push((a, b, inside)),
            // Fire resistance is given once per bot (see `fuzz_bot`).
            9 if on("lava") => fills.push((pool.0, pool.1, "lava")),
            // The base under the water makes the column; placing it after the water updates the cells above.
            10 if on("bubbles") => fills.extend([(pool.0, pool.1, "water"), ([pool.0[0], -depth - 1, pool.0[2]], [pool.1[0], -depth - 1, pool.1[2]], base)]),
            // 1.5 blocks of headroom over a slab: sneaking fits, standing does not.
            11 if on("ceiling") => fills.extend([(a, b, "smooth_stone_slab"), ([a[0], 2, a[2]], [b[0], 2, b[2]], "stone")]),
            12 if on("shapes") => fills.push((a, [b[0], top, b[2]], shape)),
            _ => {}
        }
    }
    fills.extend([([0, 0, 0], [0, 1, 0], "air"), ([0, -1, 0], [0, -1, 0], "stone")]);
    fills
}

/// Holds random controls for random spans, turning as it goes and steering back inside the area.
async fn drive(bot: &mut Bot, pad: &Pad, rng: &mut Rng, ticks: u32) -> Result<(), Error> {
    let origin = pad.block([0, 0, 0]).map(|v| v as f32);
    let (mut left, mut turn) = (0, 0.0);
    let mut c = Controls::default();
    for _ in 0..ticks {
        let Some(feet) = bot.movement().and_then(|m| m.position()) else { break };
        let [x, y, z] = [feet[0] - origin[0], feet[1] - origin[1], feet[2] - origin[2]];
        if y < PAD_MIN[1] as f32 {
            break;
        }
        if left == 0 {
            left = rng.range(2, 21);
            turn = rng.range(-6, 7) as f32;
            c.forward = [1.0, 1.0, 1.0, 0.0, -1.0][rng.range(0, 5) as usize];
            c.strafe = [0.0, 0.0, 1.0, -1.0][rng.range(0, 4) as usize];
            c.sprint = rng.chance(40);
            c.jump = rng.chance(25);
            c.sneak = rng.chance(10);
            c.pitch = rng.range(-40, 41) as f32;
            // Draws above happen regardless, so a seed builds the same terrain with features off.
            (turn, c.forward, c.strafe, c.sprint, c.jump, c.sneak, c.pitch) = (
                if on("turn") { turn } else { 0.0 },
                if on("walk") { c.forward } else { 0.0 },
                if on("strafe") { c.strafe } else { 0.0 },
                c.sprint && on("sprint"),
                c.jump && on("jump"),
                c.sneak && on("sneak"),
                if on("pitch") { c.pitch } else { 0.0 },
            );
        }
        left -= 1;
        c.yaw += turn;
        let outside = x < AREA_MIN[0] as f32 || x > (AREA_MAX[0] + 1) as f32 || z < AREA_MIN[1] as f32 || z > (AREA_MAX[1] + 1) as f32;
        if outside {
            let centre = [(AREA_MIN[0] + AREA_MAX[0]) as f32 / 2.0, (AREA_MIN[1] + AREA_MAX[1]) as f32 / 2.0];
            // Bedrock yaw 0 faces +Z.
            c.yaw = (-(centre[0] - x)).atan2(centre[1] - z).to_degrees();
            (c.forward, c.strafe) = (1.0, 0.0);
        }
        if let Some(controls) = bot.controls() {
            *controls = c;
        }
        bot.wait_ticks(1).await?;
    }
    if let Some(controls) = bot.controls() {
        controls.stop();
    }
    bot.wait_ticks(10).await?;
    Ok(())
}

/// Whether a fuzz feature is enabled: `FUZZ_OFF` lists disabled ones (turn, walk, strafe, sprint, jump, sneak,
/// pitch, floors, obstacles, web, pools, flow, climb, honeywall, inside, lava, bubbles, ceiling, shapes), for
/// bisecting a mismatch to its cause.
fn on(feature: &str) -> bool {
    std::env::var("FUZZ_OFF").map_or(true, |off| !off.split(',').any(|f| f == feature))
}

/// xorshift64*: reproducible runs from a printed seed, without a dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in `lo..hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u64) as i32
    }

    fn chance(&mut self, percent: i32) -> bool {
        self.range(0, 100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() as i32) as usize]
    }
}
