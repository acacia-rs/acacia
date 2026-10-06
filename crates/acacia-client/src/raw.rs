//! A signaled NetherNet connection with no session on it: whole game batches in and out, for a
//! caller that carries another party's session over the link (acacia-mitm relays the game's to a
//! realm or a friend's world). Dialing is [`signaling::dial`], the same as a [`crate::Client`]'s;
//! a task then keeps the socket, the connection's timers, the TURN allocation and the signaling
//! socket going until the link is dropped or the host closes.

use std::time::{Duration, Instant};

use acacia_nethernet::{Event as NetEvent, Identity};
use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::friend::FriendJoin;
use crate::signaling::{self, Dialed, SignalingTarget};
use crate::socks5::Socks5Proxy;
use crate::ConnectError;

/// As the driver's: above the WebRTC (1200-byte) packets.
const RECV_BUFFER: usize = 2048;

/// Dropping it closes the connection.
pub struct RawLink {
    out: mpsc::UnboundedSender<Bytes>,
    incoming: mpsc::UnboundedReceiver<Bytes>,
}

impl RawLink {
    /// Dials `target` as `identity`, whose key must be the one that signs the Login sent over the link.
    pub async fn dial(target: &SignalingTarget, identity: &Identity, proxy: Option<&Socks5Proxy>) -> Result<RawLink, ConnectError> {
        Ok(RawLink::over(signaling::dial(target, identity, proxy).await?, None))
    }

    /// Dials a friend's world joined with [`crate::join_friend_world`], and stays in its Xbox session
    /// for as long as the link lives. The Login sent over the link needs [`FriendJoin::nonce`].
    pub async fn dial_friend(join: FriendJoin, identity: &Identity, proxy: Option<&Socks5Proxy>) -> Result<RawLink, ConnectError> {
        let dialed = signaling::dial(&join.target, identity, proxy).await?;
        Ok(RawLink::over(dialed, Some(join.session)))
    }

    fn over(dialed: Dialed, session: Option<oneshot::Sender<()>>) -> RawLink {
        let (out, from_caller) = mpsc::unbounded_channel();
        let (to_caller, incoming) = mpsc::unbounded_channel();
        tokio::spawn(pump(dialed, from_caller, to_caller, session));
        RawLink { out, incoming }
    }

    /// Queues one batch for the host; false once the connection has ended.
    pub fn send(&self, batch: Bytes) -> bool {
        self.out.send(batch).is_ok()
    }

    /// The next batch from the host; `None` once the connection has ended.
    pub async fn recv(&mut self) -> Option<Bytes> {
        self.incoming.recv().await
    }
}

/// `_session` keeps a friend's Xbox session joined until the pump ends.
async fn pump(
    mut dialed: Dialed,
    mut from_caller: mpsc::UnboundedReceiver<Bytes>,
    to_caller: mpsc::UnboundedSender<Bytes>,
    _session: Option<oneshot::Sender<()>>,
) {
    let mut buf = vec![0u8; RECV_BUFFER];
    'link: loop {
        let now = Instant::now();
        while let Some(batch) = dialed.wire.conn.poll_message() {
            if to_caller.send(batch).is_err() {
                break 'link;
            }
        }
        while let Some(event) = dialed.wire.conn.poll_event() {
            if let NetEvent::Closed(reason) = event {
                tracing::debug!("raw link closed: {reason}");
                return;
            }
        }
        flush(&mut dialed, now).await;
        let deadline = dialed.wire.poll_timeout().unwrap_or(now + Duration::from_secs(60));
        tokio::select! {
            received = dialed.transport.recv_from(&mut buf) => match received {
                Ok((range, source)) => dialed.wire.handle_datagram(Instant::now(), source, &buf[range]),
                Err(e) => {
                    tracing::debug!("raw link socket: {e}");
                    break 'link;
                }
            },
            batch = from_caller.recv() => match batch {
                Some(batch) => dialed.wire.conn.send(batch, Instant::now()),
                None => break 'link,
            },
            _ = tokio::time::sleep_until(deadline.into()) => dialed.wire.handle_timeout(Instant::now()),
        }
    }
    let now = Instant::now();
    dialed.wire.close(now);
    flush(&mut dialed, now).await;
}

async fn flush(dialed: &mut Dialed, now: Instant) {
    while let Some((target, datagram)) = dialed.wire.poll_datagram(now) {
        // A host lists candidates we may not be able to reach (other families, LANs).
        if let Err(e) = dialed.transport.send_to(&datagram, target).await {
            tracing::trace!(%target, "send failed: {e}");
        }
    }
}
