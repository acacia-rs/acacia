//! Joining a server from one process: joins to one address are spaced and BDS's `RecentlyConnected` refusal is
//! retried (rules: the crate docs, "Lifecycle").

use std::time::Duration;

use acacia_bot::client::ClientBuilder;
use acacia_bot::swarm::JoinQueue;
use acacia_api::{ApiError, Auth, CreateBotParams, ErrorCode};
use acacia_bot::java_client::Error as JavaError;
use acacia_bot::{Bot, BotConfig, ConnectError, DisconnectReason};
use acacia_raknet::DisconnectReason as RakNetReason;

use crate::sign_in::{Prompt, SignIns};

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

/// Joins a Java server: offline as `username`, or as the Java profile of the Microsoft account it names.
pub(crate) async fn connect_java(params: &CreateBotParams, port: u16, sign_ins: &SignIns, prompt: Prompt, config: BotConfig) -> Result<Bot, ApiError> {
    let joined = match params.auth.unwrap_or(Auth::Offline) {
        Auth::Offline => Bot::connect_java_offline(&params.host, port, &params.username, config).await,
        Auth::Microsoft => {
            let account = sign_ins.account(&params.username, params.profiles_folder.as_deref());
            let account = sign_ins.java_account(account, prompt).await?;
            Bot::connect_java(&params.host, port, &account, config).await
        }
    };
    joined.map_err(|e| {
        let code = match e {
            JavaError::Io(_) => ErrorCode::Io,
            JavaError::Auth(_) | JavaError::NeedsAccount => ErrorCode::Auth,
            _ => ErrorCode::Disconnected,
        };
        ApiError::new(code, e.to_string())
    })
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
