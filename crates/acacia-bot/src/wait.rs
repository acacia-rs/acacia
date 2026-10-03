//! Waiting for server replies while the bot keeps processing packets, for async actions.

use std::time::Duration;

use acacia_client::proto::RawPacket;

use crate::bot::Step;
use crate::{Bot, BotEvent};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ActionError {
    #[error("timed out waiting for the server")]
    Timeout,
    #[error("disconnected")]
    Disconnected,
    #[error("rejected by the server: {0}")]
    Rejected(String),
    #[error("not possible: {0}")]
    NotPossible(String),
}

enum Progress {
    Packet(RawPacket),
    Tick,
}

impl Bot {
    /// One step for an action: a disconnect ends the action (and is kept for [`Bot::next`]).
    async fn progress(&mut self) -> Result<Progress, ActionError> {
        self.survival.idle = false;
        match self.step().await {
            None => Err(ActionError::Disconnected),
            Some(Step::Disconnected(reason)) => {
                self.pending.push_back(BotEvent::Disconnected(reason));
                Err(ActionError::Disconnected)
            }
            Some(Step::Tick) => Ok(Progress::Tick),
            Some(Step::Packet(packet)) => Ok(Progress::Packet(packet)),
        }
    }

    pub(crate) fn keep_for_caller(&mut self, packet: RawPacket) {
        if self.subscribe.allows(packet.id) {
            self.pending.push_back(BotEvent::Packet(packet));
        }
    }

    /// Keeps processing packets and ticks until `check` (run after each packet is applied to the
    /// state) returns `Some`. Events meant for the caller are kept for [`Bot::next`].
    pub(crate) async fn wait_until<T>(
        &mut self,
        timeout: Duration,
        mut check: impl FnMut(&Bot, &RawPacket) -> Option<T>,
    ) -> Result<T, ActionError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let progress = tokio::time::timeout_at(deadline, self.progress()).await.map_err(|_| ActionError::Timeout)??;
            if let Progress::Packet(packet) = progress {
                let hit = check(self, &packet);
                self.keep_for_caller(packet);
                if let Some(value) = hit {
                    return Ok(value);
                }
            }
        }
    }

    /// Processes packets up to and including the next movement tick; true if `check` matched any
    /// packet on the way. Physics bots only (others never tick).
    pub(crate) async fn next_tick(&mut self, mut check: impl FnMut(&Bot, &RawPacket) -> bool) -> Result<bool, ActionError> {
        let mut hit = false;
        loop {
            match self.progress().await? {
                Progress::Tick => return Ok(hit),
                Progress::Packet(packet) => {
                    hit |= check(self, &packet);
                    self.keep_for_caller(packet);
                }
            }
        }
    }

    /// Lets `ticks` movement ticks (50 ms each) pass while processing packets.
    pub async fn wait_ticks(&mut self, ticks: u32) -> Result<(), ActionError> {
        self.pause(Duration::from_millis(50 * u64::from(ticks))).await
    }

    /// Lets `duration` pass while processing packets.
    pub(crate) async fn pause(&mut self, duration: Duration) -> Result<(), ActionError> {
        match self.wait_until(duration, |_, _| None::<()>).await {
            Ok(()) | Err(ActionError::Timeout) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
