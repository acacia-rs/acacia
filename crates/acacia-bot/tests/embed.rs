//! Driving a bot from outside its task against the fake server: events during actions (`acacia_bot::events`) and
//! the immediate lane (`acacia_bot::inbox`), snapshots (`acacia_bot::snapshot`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::events::{channel, EventReceiver, EventSink};
use acacia_bot::proto::packets::{InventorySlot, Text, TextCategory, TextContent, TextContentRaw, TextType};
use acacia_bot::proto::types::{ItemV4, ItemV4Extra, WindowIDVarint};
use acacia_bot::snapshot::SnapshotReader;
use acacia_bot::{Bot, BotConfig, BotEvent, Events, Immediate};
use acacia_testserver::{FakeServer, Script};
use tokio::time::{sleep, timeout};

async fn join(name: &str, config: BotConfig) -> (FakeServer, Bot) {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline(name).transport(TransportKind::RakNet);
    let bot = Bot::connect(builder, config).await.expect("bot joins the fake server");
    server.spawned().await.expect("script reaches PlayerSpawn");
    (server, bot)
}

fn chat_line(message: &str) -> Text {
    Text {
        needs_translation: false,
        category: TextCategory::MessageOnly,
        r#type: TextType::Raw,
        content: TextContent::Raw(TextContentRaw { message: message.into() }),
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    }
}

/// Reads `events` until the chat line `text`; returns when it arrived and the ticks seen before it.
async fn chat_from(events: &mut EventReceiver, text: &str) -> (Instant, usize) {
    let mut ticks = 0;
    loop {
        match timeout(Duration::from_secs(5), events.recv()).await {
            Ok(Some(BotEvent::Chat(m))) if m.plain() == text => return (Instant::now(), ticks),
            Ok(Some(BotEvent::Tick)) => ticks += 1,
            Ok(Some(_)) => {}
            other => panic!("no chat line {text:?}: {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sink_gets_events_while_an_action_waits() {
    let (sink, mut events) = channel(256);
    let config = BotConfig { events: Events::CHAT | Events::TICKS, event_sink: Some(sink), ..BotConfig::default() };
    let (server, mut bot) = join("SinkTester", config).await;
    let server = &server;
    let ((waited, done), (chat, ticks)) = tokio::join!(
        async { (bot.wait_ticks(40).await, Instant::now()) },
        async {
            sleep(Duration::from_millis(500)).await;
            server.send(&chat_line("mid-action"));
            chat_from(&mut events, "mid-action").await
        }
    );
    waited.expect("the wait runs to its end");
    assert!(chat < done, "the chat line arrived {:?} after the action ended", chat - done);
    assert!(ticks >= 3, "ticks during the action reach the sink: {ticks}");
    assert!(timeout(Duration::from_millis(200), bot.next()).await.is_err(), "next returns nothing with a sink");
}

/// Whether the bot has sent the chat line `text`.
fn sent_chat(server: &FakeServer, text: &str) -> bool {
    let lines = server.received().into_iter().filter_map(|r| r.packet.decode::<Text>().ok());
    lines.into_iter().any(|t| matches!(&t.content, TextContent::Chat(c) if c.message == text))
}

async fn until_sent(server: &FakeServer, text: &str) -> Instant {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sent_chat(server, text) {
        assert!(Instant::now() < deadline, "{text:?} never went out");
        sleep(Duration::from_millis(10)).await;
    }
    Instant::now()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_inbox_and_the_sender_reach_the_server_mid_action() {
    let (server, mut bot) = join("InboxTester", BotConfig::default()).await;
    let (inbox, sender) = (bot.inbox(), bot.sender());
    let server = &server;
    let ((waited, done), (by_inbox, by_sender)) = tokio::join!(
        async { (bot.wait_ticks(40).await, Instant::now()) },
        async {
            sleep(Duration::from_millis(300)).await;
            std::thread::spawn(move || assert!(inbox.send(Immediate::Chat("by inbox".into())))).join().unwrap();
            let by_inbox = until_sent(server, "by inbox").await;
            std::thread::spawn(move || assert!(sender.expect("a Bedrock bot").chat("by sender"))).join().unwrap();
            (by_inbox, until_sent(server, "by sender").await)
        }
    );
    waited.expect("the wait runs to its end");
    assert!(by_inbox < done && by_sender < done, "both went out during the action");
}

/// The next chat line `next` returns within `wait`.
async fn next_chat(bot: &mut Bot, wait: Duration) -> Option<String> {
    timeout(wait, async {
        loop {
            match bot.next().await {
                Some(BotEvent::Chat(m)) => return m.plain(),
                Some(_) => {}
                None => panic!("disconnected"),
            }
        }
    })
    .await
    .ok()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_events_mask_changes_after_connect() {
    let (server, mut bot) = join("MaskTester", BotConfig::default()).await;
    server.send(&chat_line("unsubscribed"));
    assert_eq!(next_chat(&mut bot, Duration::from_millis(500)).await, None, "no chat events before subscribing");

    bot.inbox().send(Immediate::SetEvents(Events::CHAT));
    assert_eq!(next_chat(&mut bot, Duration::from_millis(300)).await, None);
    assert_eq!(bot.events(), Events::CHAT);
    server.send(&chat_line("subscribed"));
    assert_eq!(next_chat(&mut bot, Duration::from_secs(5)).await.as_deref(), Some("subscribed"));

    bot.set_events(Events::NONE);
    server.send(&chat_line("unsubscribed again"));
    assert_eq!(next_chat(&mut bot, Duration::from_millis(500)).await, None, "unsubscribing stops them");
}

fn send_sync<T: Send + Sync>(_: &T) {}

#[allow(dead_code)]
fn host_handles_are_send_and_sync(bot: &Bot, reader: &SnapshotReader, sink: &EventSink) {
    send_sync(&bot.inbox());
    send_sync(&bot.sender());
    send_sync(&bot.cancel_handle());
    send_sync(reader);
    send_sync(sink);
}

fn stone_in_slot(slot: u32, count: u16) -> InventorySlot {
    let item = ItemV4 {
        network_id: 1,
        count,
        metadata: 0,
        has_stack_id: true,
        stack_id: Some(77),
        block_runtime_id: 0,
        extra: ItemV4Extra::Default(None),
    };
    InventorySlot { window_id: WindowIDVarint::Inventory, slot, container: None, storage_item: None, item }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshots_publish_each_tick_and_rebuild_cold_on_change() {
    let (server, bot) = join("SnapshotTester", BotConfig::default()).await;
    assert!(bot.snapshot_reader().is_none(), "off by default");
    drop((server, bot));

    let (server, mut bot) = join("SnapshotTester", BotConfig { snapshots: true, ..BotConfig::default() }).await;
    let reader = bot.snapshot_reader().expect("snapshots on");
    let elsewhere = reader.clone();
    let watcher = std::thread::spawn(move || {
        let start = Instant::now();
        let first = elsewhere.hot().tick;
        while elsewhere.hot().tick < first + 5 {
            assert!(start.elapsed() < Duration::from_secs(5), "ticks stopped at {}", elsewhere.hot().tick);
            std::thread::sleep(Duration::from_millis(5));
        }
        elsewhere.world().is_some()
    });
    let _ = timeout(Duration::from_millis(600), bot.next()).await;
    assert!(watcher.join().unwrap(), "another thread sees ticks and the world");

    let before = reader.cold();
    let _ = timeout(Duration::from_millis(200), bot.next()).await;
    assert_eq!(reader.cold().version, before.version, "no change, no rebuild");
    assert!(Arc::ptr_eq(&reader.cold(), &before), "the same Arc while nothing changes");

    server.send(&stone_in_slot(3, 5));
    let _ = timeout(Duration::from_millis(500), bot.next()).await;
    let after = reader.cold();
    assert!(after.version > before.version, "an inventory change rebuilds the cold part");
    let stack = after.inventory.iter().find(|s| s.window == 0 && s.slot == 3).expect("slot 3 in the snapshot");
    assert_eq!((stack.network_id, stack.count), (1, 5));
    assert_eq!(bot.snapshot().cold.inventory, after.inventory, "an on-demand snapshot agrees");
    assert_eq!(reader.hot().position, bot.snapshot().hot.position);
}
