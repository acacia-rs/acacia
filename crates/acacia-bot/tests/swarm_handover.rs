//! Moving bots between nodes: `remove` hands back the spec with the state the task built up, and a
//! drained swarm parks waiting bots instead of dropping them.

use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::MemoryTokenCache;
use acacia_bot::client::TransportKind;
use acacia_bot::swarm::{AccountLease, BotId, BotSpec, BotStatus, LocalLeases, Login, Policy, Swarm, SwarmEvent, Target};
use acacia_bot::Bot;
use acacia_testserver::{FakeServer, Script};
use common::wait_for;
use tokio::time::timeout;

mod common;

/// Counts the sessions it has driven.
async fn count_sessions(bot: &mut Bot, sessions: &mut u32) {
    *sessions += 1;
    while bot.next().await.is_some() {}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remove_returns_the_spec_with_its_state() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let swarm = Swarm::builder()
        .shards(1)
        .join_spacing(Duration::from_millis(10), Duration::ZERO)
        .client(|_, b| b.transport(TransportKind::RakNet))
        .start(count_sessions)
        .unwrap();
    let mut events = swarm.events();
    let target = Target::Server { address: server.addr().to_string() };
    let login = Login::Offline { name: "Mover".into() };
    swarm.add(BotSpec { id: BotId::from("m"), login, target, proxy: None, state: 0 }).unwrap();
    wait_for(&mut events, |e| matches!(e, SwarmEvent::Spawned { .. })).await;

    let spec = timeout(Duration::from_secs(10), swarm.remove(&BotId::from("m"))).await.unwrap().expect("spec back");
    assert_eq!(spec.state, 1, "the state the task left");
    assert!(swarm.snapshot().bots.is_empty());
    swarm.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drained_swarm_parks_waiting_bots_for_handover() {
    let leases = Arc::new(LocalLeases::new());
    let ttl = Duration::from_secs(600);
    leases.acquire("acct", "elsewhere", ttl).await.unwrap().unwrap();
    let swarm = Swarm::builder()
        .shards(1)
        .policy(Policy { initial: Duration::from_millis(20), max: Duration::from_millis(50), ..Policy::default() })
        .join_spacing(Duration::from_millis(10), Duration::ZERO)
        .leases(leases.clone())
        .token_cache(Arc::new(MemoryTokenCache::new()))
        .start(count_sessions)
        .unwrap();
    let mut events = swarm.events();
    let login = Login::Online { account: "acct".into() };
    let target = Target::Server { address: "127.0.0.1:9".into() };
    swarm.add(BotSpec { id: BotId::from("p"), login, target, proxy: None, state: 5 }).unwrap();
    wait_for(&mut events, |e| matches!(e, SwarmEvent::Reconnecting { .. })).await;

    swarm.drain();
    timeout(Duration::from_secs(5), async {
        while swarm.snapshot().bots[0].status != BotStatus::Parked {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("parks after its backoff");
    let spec = timeout(Duration::from_secs(5), swarm.remove(&BotId::from("p"))).await.unwrap().expect("spec back");
    assert_eq!((spec.state, spec.login), (5, Login::Online { account: "acct".into() }));
    assert!(swarm.snapshot().bots.is_empty());
    swarm.shutdown().await;
}
