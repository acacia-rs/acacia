use super::*;

const TTL: Duration = Duration::from_secs(30);

#[tokio::test(start_paused = true)]
async fn one_holder_at_a_time_until_expiry() {
    let leases = LocalLeases::new();
    let a = leases.acquire("acct", "node-a/bot", TTL).await.unwrap().unwrap();
    assert!(leases.acquire("acct", "node-b/bot", TTL).await.unwrap().is_none());
    assert_eq!(leases.acquire("acct", "node-a/bot", TTL).await.unwrap(), Some(a.clone()), "re-acquire keeps the fence");

    tokio::time::advance(TTL + Duration::from_secs(1)).await;
    let b = leases.acquire("acct", "node-b/bot", TTL).await.unwrap().unwrap();
    assert!(b.fence > a.fence);
    assert!(!leases.renew(&a, TTL).await.unwrap(), "the old holder is fenced out");
    leases.release(&a).await.unwrap();
    assert!(leases.renew(&b, TTL).await.unwrap(), "a stale release leaves the new lease alone");
}

#[tokio::test(start_paused = true)]
async fn renew_extends_and_release_frees() {
    let leases = LocalLeases::new();
    let a = leases.acquire("acct", "a", TTL).await.unwrap().unwrap();
    tokio::time::advance(TTL / 2).await;
    assert!(leases.renew(&a, TTL).await.unwrap());
    tokio::time::advance(TTL / 2 + Duration::from_secs(1)).await;
    assert!(leases.acquire("acct", "b", TTL).await.unwrap().is_none(), "renewed past the first TTL");
    leases.release(&a).await.unwrap();
    assert!(leases.acquire("acct", "b", TTL).await.unwrap().is_some());
}
