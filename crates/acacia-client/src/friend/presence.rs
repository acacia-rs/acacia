//! Membership in a friend's Xbox session for as long as the game connection lives: the RTA socket
//! MPSD ties the member to, reconnected (and re-registered) if it drops, and a leave at the end.

use std::time::Duration;

use acacia_auth::{Account, FriendWorld, SessionRef, RTA_CONNECTIONS_URI};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use super::rta::{self, Frame, Rta};
use super::FriendJoinError;
use crate::socks5::Socks5Proxy;

const RECONNECT_DELAYS: [Duration; 3] = [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(4)];

pub(crate) struct Presence {
    /// The session document the join returned.
    pub session: Value,
    /// One message per RTA shoulder tap or resync: the session may have changed.
    pub changes: mpsc::UnboundedReceiver<()>,
    /// Dropping it leaves the session and closes RTA.
    pub close: oneshot::Sender<()>,
}

/// Opens RTA, joins `world`'s session tied to it, publishes the activity and keeps both alive.
pub(crate) async fn join(account: Account, world: &FriendWorld, proxy: Option<Socks5Proxy>) -> Result<Presence, FriendJoinError> {
    let (rta, sub_id, connection_id) = open_rta(&account, proxy.as_ref()).await?;
    let session = account.join_friend_world(world, &connection_id).await?;
    if let Err(e) = account.publish_friend_activity(&world.session).await {
        tracing::debug!("publishing friend-world activity failed: {e}");
    }
    let (changes_tx, changes) = mpsc::unbounded_channel();
    let (close, closed) = oneshot::channel();
    let held = Held { account, session: world.session.clone(), proxy, changes: changes_tx };
    tokio::spawn(held.run(rta, sub_id, closed));
    Ok(Presence { session, changes, close })
}

/// A connected RTA socket subscribed to MPSD connections: `(socket, subscription id, ConnectionId)`.
async fn open_rta(account: &Account, proxy: Option<&Socks5Proxy>) -> Result<(Rta, i64, String), FriendJoinError> {
    let headers = account.xbox_live_websocket_headers(rta::URL).await?;
    let mut rta = Rta::connect(headers, proxy).await?;
    let (sub_id, data) = rta.subscribe(RTA_CONNECTIONS_URI).await?;
    let connection_id = data.get("ConnectionId").and_then(Value::as_str).map(str::to_owned);
    let connection_id = connection_id.ok_or_else(|| FriendJoinError::Rta(format!("no ConnectionId in {data}")))?;
    Ok((rta, sub_id, connection_id))
}

struct Held {
    account: Account,
    session: SessionRef,
    proxy: Option<Socks5Proxy>,
    changes: mpsc::UnboundedSender<()>,
}

impl Held {
    async fn run(self, mut rta: Rta, mut sub_id: i64, mut closed: oneshot::Receiver<()>) {
        loop {
            tokio::select! {
                _ = &mut closed => break,
                frame = rta.next() => match frame {
                    Ok(Frame::Event { sub_id: s, .. }) if s == sub_id => drop(self.changes.send(())),
                    Ok(Frame::Resync) => drop(self.changes.send(())),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::debug!("friend-world RTA dropped: {e}");
                        match self.reconnect().await {
                            Some((socket, id)) => (rta, sub_id) = (socket, id),
                            None => {
                                let _ = (&mut closed).await;
                                return self.leave().await;
                            }
                        }
                    }
                },
            }
        }
        rta.close().await;
        self.leave().await;
    }

    /// A new RTA connection with the membership moved onto it; None after the last attempt fails.
    async fn reconnect(&self) -> Option<(Rta, i64)> {
        for delay in RECONNECT_DELAYS {
            tokio::time::sleep(delay).await;
            let opened = open_rta(&self.account, self.proxy.as_ref()).await;
            let result = match opened {
                Ok((rta, sub_id, id)) => self.account.set_friend_session_connection(&self.session, &id).await.map(|()| (rta, sub_id)).map_err(FriendJoinError::from),
                Err(e) => Err(e),
            };
            match result {
                Ok(ok) => return Some(ok),
                Err(e) => tracing::debug!("friend-world RTA reconnect failed: {e}"),
            }
        }
        None
    }

    async fn leave(&self) {
        if let Err(e) = self.account.leave_friend_world(&self.session).await {
            tracing::debug!("leaving friend-world session failed: {e}");
        }
    }
}
