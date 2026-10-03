use std::time::Duration;

use acacia_auth::{session_nonce, Account, AuthClient, FriendWorld, SessionRef};
use acacia_session::proto::PROTOCOL_VERSION;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::presence;
use super::FriendJoin;
use crate::realm::signaling_host;
use crate::{Client, ClientBuilder, ConnectError, SignalingTarget, Socks5Proxy};

/// How long to wait for the host to publish our nonce after the join.
const NONCE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum FriendJoinError {
    #[error(transparent)]
    Auth(#[from] crate::auth::Error),
    #[error(transparent)]
    Connect(#[from] ConnectError),
    #[error("Xbox RTA: {0}")]
    Rta(String),
    #[error("the world is on protocol {host}, this client speaks {PROTOCOL_VERSION}")]
    ProtocolMismatch { host: i32 },
    #[error("the world advertises no signaled connection")]
    NoConnection,
}

/// Joins `world` like [`join_friend_world`] and returns a signed-in builder pointed at it.
pub async fn friend_builder(
    auth: &AuthClient,
    account: &Account,
    world: &FriendWorld,
    proxy: Option<&Socks5Proxy>,
) -> Result<ClientBuilder, FriendJoinError> {
    let (key, credentials) = account.login_credentials().await?;
    let join = join_friend_world(auth, account, world, &credentials.xuid, proxy).await?;
    Ok(Client::builder(join.target.peer.clone()).online(credentials, key).friend(join))
}

/// Joins `world`'s Xbox session as `xuid` (the account's), waits for the host's nonce and picks the
/// signaling target, as the game does before dialing.
pub async fn join_friend_world(
    auth: &AuthClient,
    account: &Account,
    world: &FriendWorld,
    xuid: &str,
    proxy: Option<&Socks5Proxy>,
) -> Result<FriendJoin, FriendJoinError> {
    if world.protocol != 0 && world.protocol != PROTOCOL_VERSION {
        return Err(FriendJoinError::ProtocolMismatch { host: world.protocol });
    }
    let token = account.service_token().await?.authorization_header;
    let host = signaling_host(auth).await?;
    let target = world
        .connections
        .iter()
        .find_map(|c| SignalingTarget::from_friend(c, token.clone(), &host))
        .ok_or(FriendJoinError::NoConnection)?;
    let mut presence = presence::join(account.clone(), world, proxy.cloned()).await?;
    let nonce = match session_nonce(&presence.session, xuid) {
        Some(nonce) => Some(nonce),
        None => await_nonce(account, &world.session, xuid, &mut presence.changes).await,
    };
    Ok(FriendJoin { target, nonce, session: presence.close })
}

/// Re-reads the session on each RTA tap until the host publishes our nonce; None on timeout.
async fn await_nonce(account: &Account, session: &SessionRef, xuid: &str, changes: &mut mpsc::UnboundedReceiver<()>) -> Option<String> {
    let deadline = Instant::now() + NONCE_TIMEOUT;
    loop {
        let tapped = tokio::time::timeout_at(deadline, changes.recv()).await;
        let found = match account.friend_session(session).await {
            Ok(doc) => session_nonce(&doc, xuid),
            Err(e) => {
                tracing::debug!("reading the friend-world session failed: {e}");
                None
            }
        };
        if found.is_some() {
            return found;
        }
        if !matches!(tapped, Ok(Some(()))) {
            tracing::warn!("the host published no nonce; joining without one");
            return None;
        }
    }
}
