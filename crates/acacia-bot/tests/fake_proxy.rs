//! A physics bot joining through a gophertunnel-style proxy (Oomph), which holds every backend packet,
//! chunks included, until the client sends SetLocalPlayerAsInitialized.

use std::time::Duration;

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::proto::packets::{LevelChunk, PlayStatus, SetLocalPlayerAsInitialized};
use acacia_bot::proto::Packet;
use acacia_bot::{Bot, BotConfig};
use acacia_testserver::{FakeServer, Script, Step};

fn is(step: &Step, id: u32) -> bool {
    matches!(step, Step::Send { packet, .. } if packet.first() == Some(&(id as u8)))
}

/// The bundled BDS join with every chunk moved behind an initialisation gate right after PlayerSpawn.
fn chunks_held_until_initialised() -> Script {
    let mut script = Script::bds_spawn();
    script.steps.retain(|s| *s != Step::Wait(SetLocalPlayerAsInitialized::ID));
    let (chunks, mut rest): (Vec<Step>, Vec<Step>) = script.steps.into_iter().partition(|s| is(s, LevelChunk::ID));
    assert!(!chunks.is_empty(), "script sends chunks");
    let spawn = rest.iter().position(|s| matches!(s, Step::Send { packet, .. } if packet[..] == PLAYER_SPAWN)).expect("script spawns");
    rest.splice(spawn + 1..spawn + 1, std::iter::once(Step::Wait(SetLocalPlayerAsInitialized::ID)).chain(chunks));
    script.steps = rest;
    script
}

/// PlayStatus (2) PlayerSpawn (3, i32 big endian).
const PLAYER_SPAWN: [u8; 5] = [PlayStatus::ID as u8, 0, 0, 0, 3];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn physics_bot_leaves_the_loading_screen_without_terrain() {
    let server = FakeServer::start(chunks_held_until_initialised()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline("ProxyTester").transport(TransportKind::RakNet);
    let config = BotConfig { physics: true, ..Default::default() };
    let mut bot = Bot::connect(builder, config).await.expect("bot joins the fake server");
    let _ = tokio::time::timeout(Duration::from_secs(8), async { while bot.next().await.is_some() {} }).await;
    let got = server.received();
    let init = got.iter().find(|r| r.packet.id == SetLocalPlayerAsInitialized::ID).expect("bot initialised with no terrain");
    let spawn = server.spawned_after_login().expect("server reached PlayerSpawn");
    assert!(init.t >= spawn + Duration::from_secs(2), "initialised {:?} after login, spawn {spawn:?}", init.t);
    assert_eq!(got.iter().filter(|r| r.packet.id == SetLocalPlayerAsInitialized::ID).count(), 1);
}
