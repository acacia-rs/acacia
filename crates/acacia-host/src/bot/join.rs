//! Joining a server from one process: joins to one address are spaced and BDS's `RecentlyConnected` refusal is
//! retried (rules: the crate docs, "Lifecycle").

use std::time::Duration;

use acacia_bot::client::ClientBuilder;
use acacia_bot::swarm::JoinQueue;
use acacia_bot::{Bot, BotConfig, ConnectError, DisconnectReason};
use acacia_raknet::DisconnectReason as RakNetReason;

/// Retries after a `RecentlyConnected` refusal, waiting 1, 2, 4 and 8 s plus a queue slot (about 16 s in all).
const RETRIES: u32 = 4;
const FIRST_BACKOFF: Duration = Duration::from_secs(1);

/// [`Bot::connect`] in `address`'s next [`JoinQueue`] slot, retrying a `RecentlyConnected` refusal.
pub(crate) async fn connect(
    joins: &JoinQueue,
    address: &str,
    builder: impl Fn() -> ClientBuilder,
    config: &BotConfig,
) -> Result<Bot, ConnectError> {
    let mut retries = 0;
    loop {
        tokio::time::sleep_until(joins.reserve(address)).await;
        match Bot::connect(builder(), config.clone()).await {
            Err(e) => match backoff(&e, retries) {
                Some(wait) => {
                    retries += 1;
                    tokio::time::sleep(wait).await;
                }
                None => return Err(e),
            },
            bot => return bot,
        }
    }
}

/// How long to wait before retry `retries + 1` after `e`; `None` to give up.
fn backoff(e: &ConnectError, retries: u32) -> Option<Duration> {
    let refused = matches!(e, ConnectError::Disconnected(DisconnectReason::Transport(RakNetReason::RecentlyConnected)));
    (refused && retries < RETRIES).then(|| FIRST_BACKOFF * 2u32.pow(retries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_recently_connected_is_retried_with_doubling_waits() {
        let refused = ConnectError::Disconnected(DisconnectReason::Transport(RakNetReason::RecentlyConnected));
        let waits: Vec<_> = (0..=RETRIES).map(|n| backoff(&refused, n).map(|d| d.as_secs())).collect();
        assert_eq!(waits, [Some(1), Some(2), Some(4), Some(8), None]);
        let full = ConnectError::Disconnected(DisconnectReason::Transport(RakNetReason::ServerFull));
        assert_eq!(backoff(&full, 0), None);
        assert_eq!(backoff(&ConnectError::Timeout, 0), None);
    }
}
