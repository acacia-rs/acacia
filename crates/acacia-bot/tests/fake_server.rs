//! An idle bot joining a replayed BDS session (acacia-testserver), checked against the vanilla join
//! timeline (docs/research: vanilla-capture-2026-10-01, blob-cache).
//! `FAKE_CAPTURE=<file.jsonl>` also writes what the bot sent, for `capdiff` against a vanilla capture.

use std::time::Duration;

use acacia_bot::client::{Client, TransportKind};
use acacia_bot::proto::packets::{
    ClientCacheBlobStatus, ClientCacheStatus, ClientCameraAimAssist, ClientMovementPredictionSync, ClientToServerHandshake, PlayerAction, PlayerAuthInput,
    RequestChunkRadius, ResourcePackClientResponse, ServerboundLoadingScreen, SetLocalPlayerAsInitialized, SubchunkRequest,
};
use acacia_bot::proto::types::Action;
use acacia_bot::proto::Packet;
use acacia_bot::{Bot, BotConfig};
use acacia_testserver::{FakeServer, Received, Script};

fn first(got: &[Received], id: u32) -> &Received {
    got.iter().find(|r| r.packet.id == id).unwrap_or_else(|| panic!("bot never sent packet {id}"))
}

fn ms(r: &Received) -> u128 {
    r.t.as_millis()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_bot_follows_the_vanilla_join_timeline() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let builder = Client::builder(server.addr().to_string()).offline("FakeTester").transport(TransportKind::RakNet);
    let mut bot = Bot::connect(builder, BotConfig::default()).await.expect("bot joins the fake server");
    // Long enough for ClientMovementPredictionSync, ~9.9 s after the spawn PlayerAction.
    let _ = tokio::time::timeout(Duration::from_secs(14), async { while bot.next().await.is_some() {} }).await;
    if let Some(path) = std::env::var_os("FAKE_CAPTURE") {
        server.write_capture(path.as_ref()).unwrap();
    }
    let got = server.received();
    let spawn = server.spawned_after_login().expect("server reached PlayerSpawn").as_millis();

    // Login replies after vanilla's reaction times (session/deferred.rs).
    let handshake = ms(first(&got, ClientToServerHandshake::ID));
    assert!((30..=120).contains(&handshake), "handshake reply after {handshake} ms");
    let cache = first(&got, ClientCacheStatus::ID);
    assert!(cache.packet.decode::<ClientCacheStatus>().unwrap().enabled, "blob cache reported on");
    let packs: Vec<&Received> = got.iter().filter(|r| r.packet.id == ResourcePackClientResponse::ID).collect();
    assert_eq!(packs.len(), 2);
    assert!(ms(cache) < ms(packs[0]), "cache status before the first pack reply, as vanilla");
    first(&got, RequestChunkRadius::ID);

    // Nothing about blobs before spawn; the first status ~400 ms after it.
    let status = ms(first(&got, ClientCacheBlobStatus::ID));
    assert!(status >= spawn + 300, "first blob status {status} ms, spawn {spawn} ms");

    // The loading-screen sequence (spawn.rs): screen start/end around the first inputs, then initialised.
    let screens: Vec<u128> = got.iter().filter(|r| r.packet.id == ServerboundLoadingScreen::ID).map(ms).collect();
    assert_eq!(screens.len(), 2);
    let inputs: Vec<u128> = got.iter().filter(|r| r.packet.id == PlayerAuthInput::ID).map(ms).collect();
    assert!(inputs[0] < screens[1], "inputs start on the loading screen");
    assert!(ms(first(&got, SetLocalPlayerAsInitialized::ID)) >= screens[1]);

    // ~20 inputs a second.
    let mut gaps: Vec<u128> = inputs.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    assert!((35..=65).contains(&median), "median input gap {median} ms");

    // Sub-chunks requested; no PlayerAction(Respawn) at spawn (docs/research/vanilla-actions-2026-10-02.md,
    // "Join / spawn"); prediction sync ~9.9 s after the bot settles (its cleared aim assist).
    first(&got, SubchunkRequest::ID);
    assert!(!got.iter().any(|r| r.packet.decode::<PlayerAction>().is_ok_and(|a| a.action == Action::Respawn)));
    let sync = ms(first(&got, ClientMovementPredictionSync::ID)) - ms(first(&got, ClientCameraAimAssist::ID));
    assert!((9_400..=10_600).contains(&sync), "prediction sync {sync} ms after settling");
}
