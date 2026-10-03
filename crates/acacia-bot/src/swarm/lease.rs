use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;

/// One live session per online account across every node sharing this store. See docs/swarm.md.
#[async_trait::async_trait]
pub trait AccountLease: Send + Sync {
    /// `Ok(None)` while another holder's lease on `account` is live. A holder that already has
    /// the lease gets it back, extended.
    async fn acquire(&self, account: &str, holder: &str, ttl: Duration) -> Result<Option<Lease>, LeaseError>;
    /// Extends the lease; `Ok(false)` once it expired or passed to another holder.
    async fn renew(&self, lease: &Lease, ttl: Duration) -> Result<bool, LeaseError>;
    async fn release(&self, lease: &Lease) -> Result<(), LeaseError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub account: String,
    pub holder: String,
    /// Grows every time the account changes holder; anything that must reject a stale holder
    /// (a fencing token) can compare it.
    pub fence: u64,
}

#[derive(Debug, thiserror::Error)]
#[error("account lease: {0}")]
pub struct LeaseError(Box<dyn std::error::Error + Send + Sync>);

impl LeaseError {
    pub fn new(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(error.into())
    }
}

/// Leases within one process: the default, which keeps two specs on one node from sharing an
/// account. Nodes that share accounts need a shared store instead.
#[derive(Default)]
pub struct LocalLeases {
    state: Mutex<Local>,
}

#[derive(Default)]
struct Local {
    held: HashMap<String, (Lease, Instant)>,
    fence: u64,
}

impl LocalLeases {
    pub fn new() -> Self {
        Self::default()
    }

    fn live<'a>(local: &'a mut Local, lease: &Lease) -> Option<&'a mut Instant> {
        match local.held.get_mut(&lease.account) {
            Some((held, until)) if held == lease && *until > Instant::now() => Some(until),
            _ => None,
        }
    }
}

#[async_trait::async_trait]
impl AccountLease for LocalLeases {
    async fn acquire(&self, account: &str, holder: &str, ttl: Duration) -> Result<Option<Lease>, LeaseError> {
        let now = Instant::now();
        let mut local = self.state.lock().expect("leases lock poisoned");
        if let Some((held, until)) = local.held.get_mut(account).filter(|(_, until)| *until > now) {
            if held.holder != holder {
                return Ok(None);
            }
            *until = now + ttl;
            return Ok(Some(held.clone()));
        }
        local.fence += 1;
        let lease = Lease { account: account.to_owned(), holder: holder.to_owned(), fence: local.fence };
        local.held.insert(account.to_owned(), (lease.clone(), now + ttl));
        Ok(Some(lease))
    }

    async fn renew(&self, lease: &Lease, ttl: Duration) -> Result<bool, LeaseError> {
        let mut local = self.state.lock().expect("leases lock poisoned");
        Ok(Self::live(&mut local, lease).map(|until| *until = Instant::now() + ttl).is_some())
    }

    async fn release(&self, lease: &Lease) -> Result<(), LeaseError> {
        let mut local = self.state.lock().expect("leases lock poisoned");
        if local.held.get(&lease.account).is_some_and(|(held, _)| held == lease) {
            local.held.remove(&lease.account);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
