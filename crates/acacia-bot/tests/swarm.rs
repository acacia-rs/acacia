//! A swarm across several fake servers (acacia-testserver serves one client each): placement over
//! shards, task exit, removal, invalid specs and shutdown.

use std::time::Duration;

use acacia_bot::swarm::{AddError, BotId, BotSpec, BotStatus, Login, Swarm, SwarmEvent, Target};
use acacia_bot::client::TransportKind;
use acacia_bot::Bot;
use acacia_testserver::{FakeServer, Script};
use common::wait_for;
use tokio::time::timeout;

mod common;

#[derive(Clone, Copy)]
enum Role {
    Idle,
    /// Returns from the task right after spawning.
    Quit,
}

fn spec(id: &str, address: String, role: Role) -> BotSpec<Role> {
    let login = Login::Offline { name: format!("Swarm{id}") };
    BotSpec { id: BotId::from(id), login, target: Target::Server { address }, proxy: None, state: role }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn swarm_runs_bots_across_servers_and_shards() {
    let servers = [FakeServer::start(Script::bds_spawn()).await.unwrap(), FakeServer::start(Script::bds_spawn()).await.unwrap(), FakeServer::start(Script::bds_spawn()).await.unwrap()];
    let swarm = Swarm::builder()
        .shards(2)
        .shard_fill(1)
        .join_spacing(Duration::from_millis(10), Duration::ZERO)
        .client(|_, b| b.transport(TransportKind::RakNet))
        .start(async |bot: &mut Bot, role: &mut Role| {
            if matches!(role, Role::Idle) {
                while bot.next().await.is_some() {}
            }
        })
        .unwrap();
    let mut events = swarm.events();

    swarm.add(spec("a", servers[0].addr().to_string(), Role::Idle)).unwrap();
    swarm.add(spec("b", servers[1].addr().to_string(), Role::Idle)).unwrap();
    for _ in 0..2 {
        wait_for(&mut events, |e| matches!(e, SwarmEvent::Spawned { .. })).await;
    }
    let snap = swarm.snapshot();
    assert!(snap.bots.iter().all(|b| b.status == BotStatus::Online), "{snap:?}");
    assert_eq!(snap.shard_load, vec![1, 1], "spread over both shards");
    assert!(matches!(swarm.add(spec("a", servers[2].addr().to_string(), Role::Idle)), Err(AddError::Duplicate(_))));

    // A task that returns while connected ends the bot instead of reconnecting.
    swarm.add(spec("quitter", servers[2].addr().to_string(), Role::Quit)).unwrap();
    let removed = wait_for(&mut events, |e| matches!(e, SwarmEvent::Removed { .. })).await;
    assert_eq!(removed, SwarmEvent::Removed { id: BotId::from("quitter") });

    // Specs that cannot work fail without retrying, and stay listed until removed.
    let mut online = spec("online", servers[2].addr().to_string(), Role::Idle);
    online.login = Login::Online { account: "nobody".into() };
    swarm.add(online).unwrap();
    let failed = wait_for(&mut events, |e| matches!(e, SwarmEvent::Failed { .. })).await;
    assert!(matches!(&failed, SwarmEvent::Failed { error, .. } if error.contains("token cache")), "{failed:?}");
    let failed = swarm.remove(&BotId::from("online")).await.expect("a failed bot keeps its spec");
    assert_eq!(failed.login, Login::Online { account: "nobody".into() });
    assert!(swarm.remove(&BotId::from("online")).await.is_none());

    let a = timeout(Duration::from_secs(10), swarm.remove(&BotId::from("a"))).await.expect("remove finishes");
    assert_eq!(a.map(|s| s.id), Some(BotId::from("a")));
    assert_eq!(swarm.snapshot().bots.len(), 1, "gone by the time remove returns");

    timeout(Duration::from_secs(10), swarm.shutdown()).await.expect("shutdown finishes");
    assert!(swarm.snapshot().bots.is_empty());
    assert!(matches!(swarm.add(spec("late", servers[2].addr().to_string(), Role::Idle)), Err(AddError::Draining)));
}
