//! Client container closes, sequenced for Geyser.
//!
//! Geyser answers a client `ContainerClose` with an echo (`server: false`) and then a server close
//! (`server: true`) for the same window id, which the client must acknowledge. Geyser reuses window
//! ids, so an ack sent after the next command reaches Geyser once that command's window has taken
//! the id, and closes it (no window, or a GUI that vanishes). A client close therefore settles when
//! the server close arrives (the bot acks it while applying that packet), [`ECHO_GRACE`] after an
//! echo with no server close (vanilla servers only echo), or [`SETTLE_TIMEOUT`] after sending.
//! Commands that open windows should wait for [`Bot::container_settled`]; [`Bot::close_container`]
//! returns settled. Server-initiated closes are acked before any later packet is applied, so they
//! never need waiting for. An ack that never arrives wedges Geyser (no GUI command opens anything);
//! closing the last window id again ([`Bot::close_window`]) unsticks it.

use std::time::{Duration, Instant};

use acacia_client::proto::packets::ContainerClose;
use acacia_client::proto::types::{WindowID, WindowType};
use acacia_client::proto::RawPacket;

use crate::{ActionError, Bot};

pub(crate) const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const ECHO_GRACE: Duration = Duration::from_millis(250);

/// A client close not yet settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingClose {
    window_id: i32,
    sent: Instant,
    echoed: Option<Instant>,
}

impl PendingClose {
    pub(crate) fn new(window_id: i32, sent: Instant) -> Self {
        Self { window_id, sent, echoed: None }
    }

    /// When it settles if no server close comes first.
    pub(crate) fn settles_at(&self) -> Instant {
        let timeout = self.sent + SETTLE_TIMEOUT;
        self.echoed.map_or(timeout, |at| (at + ECHO_GRACE).min(timeout))
    }

    /// Feeds a `ContainerClose` from the server; true once it settles the close.
    pub(crate) fn on_close(&mut self, close: &ContainerClose, now: Instant) -> bool {
        if close.window_id.to_raw() as i32 != self.window_id {
            return false;
        }
        if !close.server {
            self.echoed.get_or_insert(now);
        }
        close.server
    }
}

impl Bot {
    /// Acks a server close (vanilla does; Geyser opens no further window until it gets the ack) and
    /// advances the pending client close.
    pub(crate) fn on_container_close(&mut self, packet: &RawPacket) {
        let Ok(close) = packet.decode::<ContainerClose>() else { return };
        if close.server {
            self.client.send(&ContainerClose { server: false, ..close });
        }
        if self.closing.as_mut().is_some_and(|c| c.on_close(&close, Instant::now())) {
            self.closing = None;
        }
    }

    /// The window id of a client close still settling (see the module docs).
    pub fn closing_window(&self) -> Option<i32> {
        self.closing.filter(|c| c.settles_at() > Instant::now()).map(|c| c.window_id)
    }

    /// Waits until no client close is settling, so the next command cannot have its window closed
    /// by a late ack. Returns at once when none is.
    pub async fn container_settled(&mut self) -> Result<(), ActionError> {
        self.settle_close().await.map(|_| ())
    }

    /// Closes window `window_id` whether or not it is tracked as open, and waits for it to settle.
    /// Re-closing the last window ([`crate::state::Containers::last_window_id`]) unsticks Geyser when a
    /// GUI command opens nothing. `ActionError::Timeout` if the server answered nothing.
    pub async fn close_window(&mut self, window_id: i32) -> Result<(), ActionError> {
        self.container_settled().await?;
        self.send_close(window_id, ContainerClose { window_id: WindowID::from_raw(i64::from(window_id)), window_type: WindowType::None, server: false });
        self.await_close().await
    }

    /// Sends a client close and tracks it until it settles; `tracked_id` is the id the server answers with.
    pub(crate) fn send_close(&mut self, tracked_id: i32, packet: ContainerClose) {
        self.closing = Some(PendingClose::new(tracked_id, Instant::now()));
        self.client.send(&packet);
    }

    /// Waits for the close just sent; `Timeout` if neither an echo nor a server close came.
    pub(crate) async fn await_close(&mut self) -> Result<(), ActionError> {
        if self.settle_close().await? { Ok(()) } else { Err(ActionError::Timeout) }
    }

    /// Processes packets until the pending close settles; false if it timed out unanswered.
    async fn settle_close(&mut self) -> Result<bool, ActionError> {
        loop {
            let Some(pending) = self.closing else { return Ok(true) };
            let until = pending.settles_at();
            let now = Instant::now();
            if until <= now {
                self.closing = None;
                return Ok(pending.echoed.is_some());
            }
            let changed = |bot: &Bot, _: &RawPacket| (bot.closing.map(|c| c.settles_at()) != Some(until)).then_some(());
            match self.wait_until(until - now, changed).await {
                Ok(()) | Err(ActionError::Timeout) => {}
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(id: i32, server: bool) -> ContainerClose {
        ContainerClose { window_id: WindowID::from_raw(i64::from(id)), window_type: WindowType::Container, server }
    }

    #[test]
    fn server_close_for_the_window_settles_it() {
        let t0 = Instant::now();
        let mut pending = PendingClose::new(3, t0);
        assert_eq!(pending.settles_at(), t0 + SETTLE_TIMEOUT);
        assert!(!pending.on_close(&close(4, true), t0), "another window's close");
        assert!(!pending.on_close(&close(3, false), t0 + Duration::from_millis(40)), "the echo");
        assert_eq!(pending.settles_at(), t0 + Duration::from_millis(40) + ECHO_GRACE);
        assert!(!pending.on_close(&close(3, false), t0 + Duration::from_millis(90)), "a second echo keeps the first time");
        assert_eq!(pending.settles_at(), t0 + Duration::from_millis(40) + ECHO_GRACE);
        assert!(pending.on_close(&close(3, true), t0 + Duration::from_millis(60)));
    }

    #[test]
    fn echo_grace_never_outlasts_the_timeout() {
        let t0 = Instant::now();
        let mut pending = PendingClose::new(1, t0);
        pending.on_close(&close(1, false), t0 + SETTLE_TIMEOUT - Duration::from_millis(10));
        assert_eq!(pending.settles_at(), t0 + SETTLE_TIMEOUT);
    }
}
