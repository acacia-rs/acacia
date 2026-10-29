//! Cancelled and dropped actions against the fake server (contract: `acacia_bot::cancel`). Physics bots stand on a
//! stone floor laid over the replayed spawn's all-air terrain.

use std::time::{Duration, Instant};

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::pathfind::Goal;
use acacia_bot::proto::packets::UpdateSubchunkBlocks;
use acacia_bot::proto::types::{BlockCoordinates, BlockUpdate, BlockUpdateTransitionType};
use acacia_bot::{ActionError, Bot, BotConfig};
use acacia_physics::BlockPos;
use acacia_testserver::{FakeServer, Script};
use tokio::time::{sleep, timeout};

/// Floor half-width around the spawn cell.
const FLOOR: i32 = 10;
/// A cancel must end the action within this long (a tick is 50 ms; the rest is a loaded test box's slack).
const PROMPT: Duration = Duration::from_millis(200);

fn send<T: Send>(_: &T) {}

#[allow(dead_code, unused_must_use)]
fn bot_and_action_futures_are_send(bot: &mut Bot) {
    send(&bot.goto(Goal::Y(0)));
    send(&bot.break_block([0, 0, 0]));
    send(&bot.next());
    send(bot);
}

async fn join(name: &str, physics: bool) -> (FakeServer, Bot) {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline(name).transport(TransportKind::RakNet);
    let bot = Bot::connect(builder, BotConfig { physics, ..BotConfig::default() }).await.expect("bot joins the fake server");
    server.spawned().await.expect("script reaches PlayerSpawn");
    (server, bot)
}

/// Polls the bot until `done` holds.
async fn until(bot: &mut Bot, secs: u64, what: &str, done: impl Fn(&Bot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while !done(bot) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        let _ = timeout(Duration::from_millis(50), bot.next()).await;
    }
}

/// The wire id the server would send for `name`.
fn wire(bot: &Bot, name: &str) -> u32 {
    let world = bot.world().expect("world tracker");
    let id = world.registry().expect("registry").find(name, "").unwrap_or_else(|| panic!("{name} not in the registry"));
    world.view().expect("view").world().wire_id(id)
}

fn set_blocks(server: &FakeServer, cells: &[BlockPos], wire_id: u32) {
    let update = |&[x, y, z]: &BlockPos| BlockUpdate {
        position: BlockCoordinates { x, y, z },
        runtime_id: wire_id,
        flags: 3,
        entity_unique_id: 0,
        transition_type: BlockUpdateTransitionType::Create,
    };
    let [x, y, z] = cells[0];
    server.send(&UpdateSubchunkBlocks { x: x >> 4, y: y >> 4, z: z >> 4, blocks: cells.iter().map(update).collect(), extra: vec![] });
}

/// Lays a stone floor under the spawn and waits until the bot stands on it; returns the feet cell.
async fn stand_on_floor(server: &FakeServer, bot: &mut Bot) -> BlockPos {
    let p = &bot.state().player.position;
    let feet = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
    let floor: Vec<BlockPos> =
        (-FLOOR..=FLOOR).flat_map(|dx| (-FLOOR..=FLOOR).map(move |dz| [feet[0] + dx, feet[1] - 1, feet[2] + dz])).collect();
    let loaded = |bot: &Bot| bot.world().and_then(|w| w.view()).is_some_and(|v| floor.iter().all(|c| v.chunk(c[0] >> 4, c[2] >> 4).is_some()));
    until(bot, 20, "the spawn chunks", loaded).await;
    set_blocks(server, &floor, wire(bot, "minecraft:stone"));
    let standing = |bot: &Bot| bot.movement().is_some_and(|m| m.is_started() && m.on_ground());
    until(bot, 20, "the bot to stand on the floor", standing).await;
    let f = position(bot);
    [f[0].floor() as i32, (f[1] + 0.01).floor() as i32, f[2].floor() as i32]
}

/// Runs `action` and cancels it after `after`; returns its result and how long it took to end after the cancel.
async fn cancelled_after<T>(bot: &mut Bot, after: Duration, action: impl AsyncFnOnce(&mut Bot) -> T) -> (T, Duration) {
    let handle = bot.cancel_handle();
    let cancelled = std::sync::Mutex::new(None);
    let (result, ()) = tokio::join!(action(bot), async {
        sleep(after).await;
        *cancelled.lock().unwrap() = Some(Instant::now());
        handle.cancel();
    });
    let at = cancelled.into_inner().unwrap().expect("the action outlived the cancel");
    (result, at.elapsed())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_ends_only_the_action_in_flight() {
    let (_server, mut bot) = join("CancelTester", false).await;
    bot.cancel_handle().cancel();
    assert_eq!(bot.wait_ticks(2).await, Ok(()), "a cancel while idle is ignored");

    let (result, late) = cancelled_after(&mut bot, Duration::from_millis(300), async |bot: &mut Bot| bot.wait_ticks(100).await).await;
    assert_eq!(result, Err(ActionError::Cancelled));
    assert!(late < PROMPT, "ended {late:?} after the cancel");
    assert_eq!(bot.wait_ticks(1).await, Err(ActionError::Cancelled), "the cancelled action cannot carry on");
    bot.abandon_action();
    assert_eq!(bot.wait_ticks(2).await, Ok(()), "the next action runs");

    let (result, _) = cancelled_after(&mut bot, Duration::from_millis(300), async |bot: &mut Bot| bot.wait_ticks(100).await).await;
    assert_eq!(result, Err(ActionError::Cancelled));
    let _ = timeout(Duration::from_millis(100), bot.next()).await;
    assert_eq!(bot.wait_ticks(2).await, Ok(()), "polling next takes the bot back too");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_goto_ends_within_a_tick() {
    let (server, mut bot) = join("GotoCancelTester", true).await;
    let feet = stand_on_floor(&server, &mut bot).await;
    let start = position(&bot);
    let goal = Goal::Block([feet[0] + FLOOR - 1, feet[1], feet[2]]);
    let (result, late) = cancelled_after(&mut bot, Duration::from_millis(600), async |bot: &mut Bot| bot.goto(goal).await).await;
    assert_eq!(result, Err(ActionError::Cancelled));
    assert!(late < PROMPT, "ended {late:?} after the cancel");
    let moved = position(&bot);
    assert!(moved[0] > start[0] + 0.5, "the bot was walking when cancelled: {moved:?} from {start:?}");
}

fn position(bot: &Bot) -> [f32; 3] {
    bot.movement().and_then(|m| m.position()).expect("movement started")
}
