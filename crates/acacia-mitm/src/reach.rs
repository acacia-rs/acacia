//! Reaching a new player's server, off the RakNet loop (raknet.rs): signing in, finding where the
//! server is (the proxy's fixed address, a transfer target, or what Realms says) and, for a realm
//! behind the signaling service, dialing it.

use std::net::SocketAddr;

use acacia_auth::{Account, LoginCredentials};
use acacia_client::{RawLink, RealmRoute, realm_route};
use acacia_nethernet::Identity;
use bytes::Bytes;
use p384::ecdsa::SigningKey;
use tokio::sync::mpsc;

use crate::proxy::Target;
use crate::transfer;

/// How a new player's server is reached.
pub enum Reach {
    Address(SocketAddr),
    /// A realm behind the signaling service, already dialed.
    Link(RawLink),
}

/// What a new player's server side is built from.
pub struct Dialed {
    pub key: SigningKey,
    pub credentials: Option<LoginCredentials>,
    pub upstream: Reach,
}

/// What a player's server sent, by the game's address; `None` when a raw link ended.
pub type Arrived = (SocketAddr, Option<Bytes>);

/// `route` is a transfer target to dial in place of `target`.
pub async fn dial(account: Option<Account>, route: Option<(String, u16)>, target: Target) -> Result<Dialed, String> {
    let key = SigningKey::random(&mut rand_core::OsRng);
    let credentials = match &account {
        Some(account) => Some(account.credentials(&key).await.map_err(|e| format!("online login: {e}"))?),
        None => None,
    };
    let named = |host: String, port: u16| async move {
        transfer::resolve(&host, port).await.map(Reach::Address).map_err(|e| format!("{host}:{port}: {e}"))
    };
    let upstream = match (route, target) {
        (Some((host, port)), _) => named(host, port).await?,
        (None, Target::Address(server)) => Reach::Address(server),
        (None, Target::Realm { auth, id }) => {
            let account = account.as_ref().ok_or("a realm needs an online account")?;
            match realm_route(&auth, account, id, None).await.map_err(|e| format!("realm {id}: {e}"))? {
                RealmRoute::Address(address) => {
                    let (host, port) = address.rsplit_once(':').ok_or_else(|| format!("realm address {address}"))?;
                    named(host.to_owned(), port.parse().map_err(|_| format!("realm address {address}"))?).await?
                }
                RealmRoute::Signaled(target) => {
                    let token = credentials.as_ref().and_then(|c| c.multiplayer_token.clone()).ok_or("the account has no MultiplayerToken")?;
                    let link = RawLink::dial(&target, &Identity::multiplayer(key.clone(), token), None).await;
                    Reach::Link(link.map_err(|e| format!("realm {id}: {e}"))?)
                }
            }
        }
    };
    Ok(Dialed { key, credentials, upstream })
}

/// Carries a raw link's batches to the loop and the loop's to the link, until either side ends:
/// the link closing, or the pair dropping its sender, after which what was queued still goes out.
pub async fn carry(mut link: RawLink, game: SocketAddr, to_loop: mpsc::UnboundedSender<Arrived>, mut from_pair: mpsc::UnboundedReceiver<Bytes>) {
    loop {
        tokio::select! {
            batch = link.recv() => {
                let ended = batch.is_none();
                if to_loop.send((game, batch)).is_err() || ended {
                    return;
                }
            }
            batch = from_pair.recv() => match batch {
                Some(batch) => drop(link.send(batch)),
                None => return,
            },
        }
    }
}
