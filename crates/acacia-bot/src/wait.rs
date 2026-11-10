//! Waiting for server replies while the bot keeps processing packets, for async actions.

use std::time::Duration;

use acacia_client::proto::RawPacket;

use crate::bot::Step;
use crate::{Bot, BotEvent};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ActionError {
    #[error("timed out waiting for the server")]
    Timeout,
    #[error("disconnected")]
    Disconnected,
    #[error("rejected by the server: {0}")]
    Rejected(String),
    #[error("not possible: {0}")]
    NotPossible(String),
    /// Nothing was sent: the target is in reach but hidden behind blocks.
    #[error("not visible: {0}")]
    NotVisible(String),
    /// [`crate::CancelHandle::cancel`] ended the action (see [`crate::cancel`]).
    #[error("cancelled")]
    Cancelled,
    /// The bot's edition has no implementation of this yet.
    #[error("not supported on Java Edition yet: {0}")]
    Unsupported(&'static str),
}

enum Progress {
    Packet(RawPacket),
    Tick,
}

impl Bot {
    /// One step for an action: a disconnect ends the action (and is kept for [`Bot::next`]), and so does a cancel
    /// (see [`crate::cancel`]).
    async fn progress(&mut self) -> Result<Progress, ActionError> {
        self.survival.idle = false;
        loop {
            self.cancel_point()?;
            let progress = match self.step().await {
                None => return Err(ActionError::Disconnected),
                Some(Step::Disconnected(reason)) => {
                    self.pending.push_back(BotEvent::Disconnected(reason));
                    return Err(ActionError::Disconnected);
                }
                Some(Step::Tick) => Progress::Tick,
                Some(Step::Idle) => continue,
                Some(Step::Packet(packet)) => {
                    self.note_echo(&packet);
                    Progress::Packet(packet)
                }
            };
            if let Err(e) = self.cancel_point() {
                if let Progress::Packet(packet) = progress {
                    self.keep_for_caller(packet);
                }
                return Err(e);
            }
            return Ok(progress);
        }
    }

    pub(crate) fn keep_for_caller(&mut self, packet: RawPacket) {
        if self.backend.bedrock().is_some_and(|b| b.subscribe.allows(packet.id)) {
            self.pending.push_back(BotEvent::Packet(packet));
        }
    }

    /// Keeps processing packets and ticks until `check` (run after each packet is applied to the
    /// state) returns `Some`. Events meant for the caller are kept for [`Bot::next`]. Java packets
    /// are applied inside the backend, so on Java `check` runs once a tick with [`no_packet`].
    pub(crate) async fn wait_until<T>(
        &mut self,
        timeout: Duration,
        mut check: impl FnMut(&Bot, &RawPacket) -> Option<T>,
    ) -> Result<T, ActionError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let progress = tokio::time::timeout_at(deadline, self.progress()).await.map_err(|_| ActionError::Timeout)??;
            let hit = match progress {
                Progress::Packet(packet) => {
                    let hit = check(self, &packet);
                    self.keep_for_caller(packet);
                    hit
                }
                Progress::Tick if self.backend.bedrock().is_none() => check(self, &no_packet()),
                Progress::Tick => None,
            };
            if let Some(value) = hit {
                return Ok(value);
            }
        }
    }

    /// Processes packets up to and including the next movement tick; true if `check` matched any
    /// packet on the way. Bedrock physics bots only (others never tick).
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

    /// Lets `ticks` movement ticks (50 ms each, `cadence::scaled`) pass while processing packets.
    pub async fn wait_ticks(&mut self, ticks: u32) -> Result<(), ActionError> {
        self.pause(crate::cadence::scaled(Duration::from_millis(50 * u64::from(ticks)))).await
    }

    /// Lets `duration` pass while processing packets.
    pub(crate) async fn pause(&mut self, duration: Duration) -> Result<(), ActionError> {
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            match tokio::time::timeout_at(deadline, self.progress()).await {
                Err(_) => return Ok(()),
                Ok(Err(e)) => return Err(e),
                Ok(Ok(Progress::Packet(packet))) => self.keep_for_caller(packet),
                Ok(Ok(Progress::Tick)) => {}
            }
        }
    }
}

/// What [`Bot::wait_until`]'s check sees on Java: an id no Bedrock packet has.
fn no_packet() -> RawPacket {
    RawPacket { id: u32::MAX, sender_subclient: 0, target_subclient: 0, body: Default::default() }
}
