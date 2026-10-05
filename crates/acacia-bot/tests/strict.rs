//! Strict mode end to end (docs/testing.md): the replayed BDS join is clean, and a packet a strict
//! peer would reject reaches the caller as a violation.

use std::time::Duration;

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::proto::packets::SetPlayerGameType;
use acacia_bot::proto::strict::Leniency;
use acacia_bot::proto::types::GameMode;
use acacia_bot::proto::DecodeError;
use acacia_bot::{Bot, BotConfig, BotEvent, Reason, Violation};
use acacia_testserver::{FakeServer, Script};

/// Violations the bot reports within `window`.
async fn violations(bot: &mut Bot, window: Duration) -> Vec<Violation> {
    let mut found = Vec::new();
    let _ = tokio::time::timeout(window, async {
        while let Some(event) = bot.next().await {
            if let BotEvent::Violation(v) = event {
                found.push(v);
            }
        }
    })
    .await;
    found
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_bot_reports_only_what_the_server_gets_wrong() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline("StrictTester").transport(TransportKind::RakNet);
    let mut bot = Bot::connect(builder, BotConfig { strict: true, ..BotConfig::default() }).await.expect("bot joins the fake server");
    server.spawned().await.expect("script reaches PlayerSpawn");

    let join = violations(&mut bot, Duration::from_secs(3)).await;
    assert!(join.is_empty(), "the BDS join has violations:\n{}", join.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n"));

    server.send(&SetPlayerGameType { gamemode: GameMode::Unknown(77) });
    let found = violations(&mut bot, Duration::from_secs(2)).await;
    let [Violation { packet, reason: Reason::Decode(e) }] = &found[..] else { panic!("expected one decode violation, got {found:?}") };
    assert_eq!(*packet, <SetPlayerGameType as acacia_bot::proto::Packet>::ID);
    assert_eq!(e.root(), &DecodeError::Lenient(Leniency::UnknownEnum { ty: "GameMode", value: 77 }));
}
