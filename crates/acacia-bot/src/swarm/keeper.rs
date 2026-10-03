use std::sync::Arc;
use std::time::Duration;

use tokio::time::{sleep_until, Instant};

use super::lease::{AccountLease, Lease};

/// One bot's account lease: held across reconnects, renewed at a third of the TTL.
pub(crate) struct Keeper {
    leases: Arc<dyn AccountLease>,
    holder: String,
    ttl: Duration,
    held: Option<Held>,
}

struct Held {
    lease: Lease,
    expires: Instant,
    renew_at: Instant,
}

impl Keeper {
    pub fn new(leases: Arc<dyn AccountLease>, holder: String, ttl: Duration) -> Self {
        Self { leases, holder, ttl, held: None }
    }

    /// `Err` says why the bot may not play `account` now (held elsewhere, or the store failed).
    pub async fn acquire(&mut self, account: &str) -> Result<(), String> {
        if self.held.as_ref().is_some_and(|h| h.lease.account == account) {
            return Ok(());
        }
        let start = Instant::now();
        match self.leases.acquire(account, &self.holder, self.ttl).await {
            Ok(Some(lease)) => {
                self.held = Some(Held { lease, expires: start + self.ttl, renew_at: start + self.ttl / 3 });
                Ok(())
            }
            Ok(None) => Err(format!("account {account} is leased by another bot")),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Renews while polled and resolves once the lease is gone; pending forever if none is held.
    /// Cancel-safe: the schedule lives in `self`, so racing this against short phases still renews.
    pub async fn lost(&mut self) {
        let Some(held) = self.held.as_mut() else { return std::future::pending().await };
        loop {
            sleep_until(held.renew_at).await;
            let start = Instant::now();
            match self.leases.renew(&held.lease, self.ttl).await {
                Ok(true) => {
                    held.expires = start + self.ttl;
                    held.renew_at = start + self.ttl / 3;
                }
                Ok(false) => break,
                Err(e) => {
                    tracing::warn!(account = %held.lease.account, error = %e, "lease renewal failed");
                    held.renew_at = Instant::now() + self.ttl / 6;
                    // Stop before expiry: past it another node may already be joining.
                    if held.renew_at >= held.expires {
                        break;
                    }
                }
            }
        }
        tracing::warn!(account = %held.lease.account, "account lease lost");
        self.held = None;
    }

    pub async fn release(&mut self) {
        if let Some(held) = self.held.take()
            && let Err(e) = self.leases.release(&held.lease).await
        {
            tracing::warn!(account = %held.lease.account, error = %e, "lease release failed; it expires by TTL");
        }
    }
}

#[cfg(test)]
mod tests;
