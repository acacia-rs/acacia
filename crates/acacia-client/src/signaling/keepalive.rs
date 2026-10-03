//! Keeps the signaling socket open after the join, as vanilla does: answers the service and pings
//! until the connection ends, then closes the socket cleanly.

use std::time::Instant;

use acacia_nethernet::signaling::SignalingSession;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

use super::ws::{self, ws_error};
use crate::ConnectError;

/// Dropping it closes the signaling socket.
pub(crate) struct Keepalive {
    _close: oneshot::Sender<()>,
}

impl Keepalive {
    pub fn spawn(socket: ws::Socket, signaling: SignalingSession) -> Self {
        let (close, closed) = oneshot::channel();
        tokio::spawn(run(socket, signaling, closed));
        Self { _close: close }
    }
}

async fn run(mut socket: ws::Socket, mut signaling: SignalingSession, mut closed: oneshot::Receiver<()>) {
    loop {
        // Signals after the join (late candidates, errors) have nothing left to act on.
        while signaling.poll_event().is_some() {}
        if let Err(e) = flush(&mut socket, &mut signaling).await {
            return tracing::debug!("signaling keepalive: {e}");
        }
        tokio::select! {
            _ = &mut closed => {
                let _ = socket.close(None).await;
                return;
            }
            frame = socket.next() => {
                if let Err(e) = handle_frame(&mut signaling, frame) {
                    return tracing::debug!("signaling keepalive: {e}");
                }
            }
            _ = tokio::time::sleep_until(signaling.poll_timeout().into()) => signaling.handle_timeout(Instant::now()),
        }
    }
}

pub(super) async fn flush(socket: &mut ws::Socket, signaling: &mut SignalingSession) -> Result<(), ConnectError> {
    while let Some(text) = signaling.poll_text() {
        tracing::trace!(%text, "signaling out");
        socket.send(Message::Text(text.into())).await.map_err(ws_error)?;
    }
    Ok(())
}

pub(super) fn handle_frame(
    signaling: &mut SignalingSession,
    frame: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> Result<(), ConnectError> {
    tracing::trace!(?frame, "signaling in");
    match frame {
        Some(Ok(Message::Text(text))) => signaling.handle_text(text.as_str()),
        Some(Ok(Message::Close(close))) => return Err(ConnectError::Signaling(format!("WebSocket closed: {close:?}"))),
        Some(Ok(_)) => {}
        Some(Err(e)) => return Err(ws_error(e)),
        None => return Err(ConnectError::Signaling("WebSocket closed".into())),
    }
    Ok(())
}
