//! Container GUIs the way Geyser serves them (titled fake blocks, echo + server close), against the fake server.

use std::time::{Duration, Instant};

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::proto::nbt::{Nbt, Value};
use acacia_bot::proto::packets::{BlockEntityData, ContainerClose, ContainerOpen, Interact, InventoryContent};
use acacia_bot::proto::types::{
    BlockCoordinates, ContainerSlotType, FullContainerName, ItemV4, ItemV4Extra, WindowID, WindowIDVarint, WindowType,
};
use acacia_bot::{ActionError, Bot, BotConfig, BotEvent, Events};
use acacia_testserver::{FakeServer, Script};

const AT: BlockCoordinates = BlockCoordinates { x: 4, y: 70, z: -9 };
const TITLE: &str = "§8Auction House (Page 1)";

async fn join(name: &str) -> (FakeServer, Bot) {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline(name).transport(TransportKind::RakNet);
    let config = BotConfig { events: Events::WINDOWS, ..BotConfig::default() };
    let bot = Bot::connect(builder, config).await.expect("bot joins the fake server");
    server.spawned().await.expect("script reaches PlayerSpawn");
    (server, bot)
}

fn item(network_id: i16) -> ItemV4 {
    let stack_id = (network_id != 0).then_some(i32::from(network_id));
    ItemV4 { network_id, count: u16::from(network_id != 0), metadata: 0, has_stack_id: stack_id.is_some(), stack_id, block_runtime_id: 0, extra: ItemV4Extra::Default(None) }
}

/// Geyser's open: the titled fake block, the open, then the contents.
fn open_gui(server: &FakeServer, window_id: WindowID) {
    let name = Value::Compound(vec![("id".into(), Value::String("Chest".into())), ("CustomName".into(), Value::String(TITLE.into()))]);
    server.send(&BlockEntityData { position: AT, nbt: Nbt { name: Default::default(), value: name }.into() });
    server.send(&ContainerOpen { window_id, window_type: WindowType::Container, coordinates: AT, runtime_entity_id: -1 });
    let window = WindowIDVarint::from_raw(window_id.to_raw());
    let container = FullContainerName { container_id: ContainerSlotType::Container, dynamic_container_id: None };
    server.send(&InventoryContent { window_id: window, input: vec![item(1), item(0), item(5)], container, storage_item: item(0) });
}

fn close(window_id: WindowID, server: bool) -> ContainerClose {
    ContainerClose { window_id, window_type: WindowType::Container, server }
}

async fn window_contents(bot: &mut Bot) -> acacia_bot::state::Container {
    let mut opened = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match bot.next().await.expect("connection open") {
                BotEvent::WindowOpened(c) => {
                    assert!(c.slots.is_empty(), "opened before its contents");
                    opened = true;
                }
                BotEvent::WindowContents(c) => {
                    assert!(opened, "contents after the open");
                    return c;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("window with contents within 5 s")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn geyser_close_is_acked_before_the_next_command() {
    let (server, mut bot) = join("GuiTester").await;
    open_gui(&server, WindowID::First);
    let window = window_contents(&mut bot).await;
    assert_eq!((window.window_id, window.title.as_deref(), window.slots.len()), (1, Some(TITLE), 3));

    let trace = bot.packet_trace();
    assert!(trace.total > 0 && trace.silent_ms.is_some(), "{trace:?}");
    assert!(trace.counts_by_name.contains_key("ContainerOpen") && trace.counts_by_name.contains_key("StartGame"), "{trace:?}");
    assert!(trace.tail.iter().any(|t| t.starts_with("InventoryContent@-")), "{:?}", trace.tail);

    // Geyser: echo at once, its own close a little later. A command sent before the ack would lose its window.
    let geyser = async {
        let sent = server.recv::<ContainerClose>().await.expect("client close");
        assert_eq!((sent.window_id, sent.server), (WindowID::First, false));
        server.send(&close(WindowID::First, false));
        tokio::time::sleep(Duration::from_millis(150)).await;
        server.send(&close(WindowID::First, true));
    };
    let (closed, ()) = tokio::join!(bot.close_container(), geyser);
    closed.expect("close settles");
    assert_eq!((bot.closing_window(), bot.state().containers.open.as_ref().map(|c| c.window_id)), (None, None));
    bot.open_inventory();

    let ack = server.recv::<ContainerClose>().await.expect("ack");
    assert_eq!((ack.window_id, ack.server), (WindowID::First, false));
    server.recv::<Interact>().await.expect("the next command after the ack");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn echo_only_close_settles_after_the_grace_and_silence_times_out() {
    let (server, mut bot) = join("EchoTester").await;
    open_gui(&server, WindowID::from_raw(2));
    window_contents(&mut bot).await;

    let vanilla = async {
        server.recv::<ContainerClose>().await.expect("client close");
        server.send(&close(WindowID::from_raw(2), false));
    };
    let start = Instant::now();
    let (closed, ()) = tokio::join!(bot.close_container(), vanilla);
    closed.expect("echo alone settles");
    let took = start.elapsed();
    assert!(took >= Duration::from_millis(200) && took < Duration::from_millis(1500), "{took:?}");

    // A late echo for an old id must not close a newer window.
    open_gui(&server, WindowID::from_raw(3));
    window_contents(&mut bot).await;
    server.send(&close(WindowID::from_raw(2), false));
    let _ = tokio::time::timeout(Duration::from_millis(300), async { while bot.next().await.is_some() {} }).await;
    assert_eq!(bot.state().containers.open.as_ref().map(|c| c.window_id), Some(3));

    let start = Instant::now();
    assert_eq!(bot.close_window(7).await, Err(ActionError::Timeout), "nothing answers window 7");
    assert!(start.elapsed() >= Duration::from_millis(1900));
    assert_eq!(bot.state().containers.last_window_id, Some(3));
}
