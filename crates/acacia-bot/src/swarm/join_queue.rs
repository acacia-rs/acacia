use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

use crate::cadence::{seed, splitmix};

/// Spaces joins to the same target `delay ± jitter` apart, across every shard, so a swarm start or a
/// server restart does not land as one burst. Different targets do not wait for each other.
pub(crate) struct JoinQueue {
    delay: Duration,
    jitter: Duration,
    next: Mutex<HashMap<String, Instant>>,
}

impl JoinQueue {
    pub fn new(delay: Duration, jitter: Duration) -> Self {
        Self { delay, jitter: jitter.min(delay), next: Mutex::new(HashMap::new()) }
    }

    /// Reserves the next free slot for `target`; the caller sleeps until it.
    pub fn reserve(&self, target: &str) -> Instant {
        let now = Instant::now();
        let mut next = self.next.lock().expect("join queue lock poisoned");
        next.retain(|_, at| *at > now);
        let slot = next.get(target).copied().unwrap_or(now);
        let spread = self.jitter.as_millis() as u64 * 2;
        let gap = self.delay - self.jitter + Duration::from_millis(noise() % (spread + 1));
        next.insert(target.to_owned(), slot + gap);
        slot
    }
}

/// Timing noise for joins and backoff.
pub(crate) fn noise() -> u64 {
    splitmix(&mut seed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn same_target_is_spaced_and_others_are_not() {
        let q = JoinQueue::new(Duration::from_millis(500), Duration::from_millis(250));
        let start = Instant::now();
        let a1 = q.reserve("a");
        let a2 = q.reserve("a");
        let b1 = q.reserve("b");
        assert_eq!(a1, start);
        assert_eq!(b1, start);
        let gap = a2 - a1;
        assert!(gap >= Duration::from_millis(250) && gap <= Duration::from_millis(750), "{gap:?}");
    }
}
