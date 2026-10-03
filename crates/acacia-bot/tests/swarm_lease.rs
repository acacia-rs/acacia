//! An online bot joins only while it holds its account's lease, and gives the lease back when it stops.

use std::sync::Arc;
use std::time::Duration;

use acacia_bot::client::auth::MemoryTokenCache;
use acacia_bot::swarm::{AccountLease, BotId, BotSpec, BotStatus, LocalLeases, Login, Swarm, SwarmEvent, Target};
use acacia_bot::Bot;
use common::wait_for;

mod common;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn online_bot_waits_for_its_account_lease() {
    let leases = Arc::new(LocalLeases::new());
    let ttl = Duration::from_secs(600);
    let elsewhere = leases.acquire("acct", "node-b/x", ttl).await.unwrap().unwrap();
    let swarm = Swarm::builder()
        .shards(1)
        .join_spacing(Duration::from_millis(10), Duration::ZERO)
        .leases(leases.clone())
        .node_id("node-a")
        .token_cache(Arc::new(MemoryTokenCache::new()))
        .start(async |_: &mut Bot, _: &mut ()| {})
        .unwrap();
    let mut events = swarm.events();
    let login = Login::Online { account: "acct".into() };
    let target = Target::Server { address: "127.0.0.1:9".into() };
    swarm.add(BotSpec { id: BotId::from("a"), login, target, proxy: None, state: () }).unwrap();

    let first = wait_for(&mut events, |e| matches!(e, SwarmEvent::Joining { .. } | SwarmEvent::Reconnecting { .. })).await;
    assert!(matches!(first, SwarmEvent::Reconnecting { .. }), "must not join while the lease is elsewhere: {first:?}");
    assert_eq!(swarm.snapshot().bots[0].status, BotStatus::AccountBusy);

    // Once free it takes the lease and tries to sign in; the empty cache fails it for good.
    leases.release(&elsewhere).await.unwrap();
    let failed = wait_for(&mut events, |e| matches!(e, SwarmEvent::Failed { .. })).await;
    assert!(matches!(&failed, SwarmEvent::Failed { error, .. } if error.contains("not signed in")), "{failed:?}");
    assert!(leases.acquire("acct", "node-b/x", ttl).await.unwrap().is_some(), "a failed bot releases its lease");
    swarm.shutdown().await;
}
