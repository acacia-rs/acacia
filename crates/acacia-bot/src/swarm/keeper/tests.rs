use std::sync::atomic::{AtomicBool, Ordering};

use tokio::time::timeout;

use super::*;
use crate::swarm::lease::{LeaseError, LocalLeases};

const TTL: Duration = Duration::from_secs(30);

/// Local leases whose renewals can be made to fail.
#[derive(Default)]
struct Flaky {
    inner: LocalLeases,
    down: AtomicBool,
}

#[async_trait::async_trait]
impl AccountLease for Flaky {
    async fn acquire(&self, account: &str, holder: &str, ttl: Duration) -> Result<Option<Lease>, LeaseError> {
        self.inner.acquire(account, holder, ttl).await
    }

    async fn renew(&self, lease: &Lease, ttl: Duration) -> Result<bool, LeaseError> {
        if self.down.load(Ordering::Relaxed) {
            return Err(LeaseError::new("store unreachable"));
        }
        self.inner.renew(lease, ttl).await
    }

    async fn release(&self, lease: &Lease) -> Result<(), LeaseError> {
        self.inner.release(lease).await
    }
}

#[tokio::test(start_paused = true)]
async fn renews_through_short_races() {
    let leases = Arc::new(LocalLeases::new());
    let mut keeper = Keeper::new(leases.clone(), "a".into(), TTL);
    keeper.acquire("acct").await.unwrap();
    // Raced against phases much shorter than the renewal interval, as the supervisor does.
    for _ in 0..200 {
        assert!(timeout(Duration::from_secs(1), keeper.lost()).await.is_err());
    }
    assert!(leases.acquire("acct", "b", TTL).await.unwrap().is_none(), "still held after 200 s");
}

#[tokio::test(start_paused = true)]
async fn gives_up_before_expiry_when_the_store_is_down() {
    let leases = Arc::new(Flaky::default());
    let mut keeper = Keeper::new(leases.clone(), "a".into(), TTL);
    keeper.acquire("acct").await.unwrap();
    leases.down.store(true, Ordering::Relaxed);
    let start = Instant::now();
    keeper.lost().await;
    assert!(start.elapsed() < TTL, "stopped after {:?}", start.elapsed());
    assert!(timeout(TTL * 10, keeper.lost()).await.is_err(), "nothing held, so never lost again");
}

#[tokio::test(start_paused = true)]
async fn busy_while_held_elsewhere_and_lost_on_takeover() {
    let leases = Arc::new(LocalLeases::new());
    let other = leases.acquire("acct", "b", TTL).await.unwrap().unwrap();
    let mut keeper = Keeper::new(leases.clone(), "a".into(), TTL);
    assert!(keeper.acquire("acct").await.unwrap_err().contains("leased"));
    leases.release(&other).await.unwrap();
    keeper.acquire("acct").await.unwrap();

    // Not polled past the TTL (a stalled shard): another holder takes over, and the next poll notices.
    tokio::time::advance(TTL * 2).await;
    leases.acquire("acct", "b", TTL).await.unwrap().unwrap();
    timeout(Duration::from_millis(1), keeper.lost()).await.expect("lost at once");
}

#[tokio::test(start_paused = true)]
async fn release_frees_the_account() {
    let leases = Arc::new(LocalLeases::new());
    let mut keeper = Keeper::new(leases.clone(), "a".into(), TTL);
    keeper.acquire("acct").await.unwrap();
    keeper.release().await;
    assert!(leases.acquire("acct", "b", TTL).await.unwrap().is_some());
}
