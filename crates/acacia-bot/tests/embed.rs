//! Driving a bot from outside its task against the fake server: events during actions (`acacia_bot::events`) and
//! the immediate lane (`acacia_bot::inbox`).

use std::time::{Duration, Instant};

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::events::{channel, EventReceiver};
use acacia_bot::proto::packets::{Text, TextCategory, TextContent, TextContentRaw, TextType};
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
            std::thread::spawn(move || assert!(sender.chat("by sender"))).join().unwrap();
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
