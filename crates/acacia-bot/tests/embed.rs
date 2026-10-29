//! Driving a bot from outside its task against the fake server: events during actions (`acacia_bot::events`).

use std::time::{Duration, Instant};

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::events::{channel, EventReceiver};
use acacia_bot::proto::packets::{Text, TextCategory, TextContent, TextContentRaw, TextType};
use acacia_bot::{Bot, BotConfig, BotEvent, Events};
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
