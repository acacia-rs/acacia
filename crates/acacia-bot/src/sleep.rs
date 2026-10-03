//! Beds. Sleeping is a right-click on the bed; the server answers by flagging the player as
//! sleeping in `SetEntityData` (or explains in chat why not), and the client confirms with
//! `PlayerAction StartSleeping`. Leaving is `StopSleeping`, sent by the player or in answer to the
//! server's `Animate WakeUp` (`crate::reflex`). Sources: docs/research/survival-signs-beds.md §5.

use std::time::{Duration, Instant};

use acacia_client::proto::packets::{Animate, PlayerAction, Text};
use acacia_client::proto::types::{Action, BlockCoordinates};
use acacia_client::proto::{Packet, RawPacket};
use acacia_physics::BlockPos;

use crate::events::ChatMessage;
use crate::human::START_SLEEP;
use crate::interact::facing_face;
use crate::reflex::Reflex;
use crate::{ActionError, Bot};

/// Packets the bed actions need beyond the trackers' (refusals arrive as chat, wake-ups as Animate).
pub(crate) const PACKETS: &[u32] = &[Text::ID, Animate::ID];
const SLEEP_TIMEOUT: Duration = Duration::from_secs(3);
const WAKE_TIMEOUT: Duration = Duration::from_secs(3);
/// Bedrock translation keys (BDS, PocketMine, PowerNukkitX) and English texts (Geyser renders Java's).
const REFUSALS: &[&str] = &[
    "tile.bed.nosleep",
    "tile.bed.notsafe",
    "tile.bed.toofar",
    "tile.bed.occupied",
    "tile.bed.notvalid",
    "sleep only at night",
    "only sleep at night",
    "may not rest now",
    "bed is occupied",
    "bed is obstructed",
    "bed is too far away",
];

impl Bot {
    pub fn is_sleeping(&self) -> bool {
        self.state.player.is_sleeping()
    }

    /// Lies down in the bed at `pos` and waits until the server shows the player sleeping.
    /// `Rejected` with the server's message when it refuses (daytime, monsters nearby, occupied,
    /// too far).
    pub async fn sleep(&mut self, pos: BlockPos) -> Result<(), ActionError> {
        if self.is_sleeping() {
            return Ok(());
        }
        if let Some((_, block)) = self.block_at(pos) {
            if !block.name.ends_with("bed") {
                return Err(ActionError::NotPossible(format!("{} at {pos:?} is not a bed", block.name)));
            }
            self.bed.lying = tick::lying_in(block.name, block.properties, pos);
        }
        self.bed.clicked_from = Some(self.state.player.position.clone());
        let face = facing_face(self.eye_position(), pos);
        self.use_item_on_block(pos, face).await?;
        let clicked = Instant::now();
        let outcome = self
            .wait_until(SLEEP_TIMEOUT, |bot, packet| {
                if bot.is_sleeping() {
                    return Some(Ok(()));
                }
                bed_refusal(packet).map(|message| Err(ActionError::Rejected(message)))
            })
            .await;
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(ActionError::Timeout) => return Err(ActionError::Rejected("the server did not put the player to bed".into())),
            Err(e) => return Err(e),
        }
        let settle = self.human.between(START_SLEEP).saturating_sub(clicked.elapsed());
        self.pause(settle).await?;
        self.client.send(&sleep_action(self.runtime_id(), Action::StartSleeping));
        self.reflexes.in_bed = true;
        Ok(())
    }

    /// Presses "leave bed" and waits until the server shows the player awake.
    pub async fn wake(&mut self) -> Result<(), ActionError> {
        if !self.is_sleeping() {
            return Err(ActionError::NotPossible("not sleeping".into()));
        }
        self.reflexes.in_bed = false;
        self.client.send(&sleep_action(self.runtime_id(), Action::StopSleeping));
        self.reflexes.schedule(0, Reflex::ClearAimAssist);
        self.wait_until(WAKE_TIMEOUT, |bot, _| (!bot.is_sleeping()).then_some(())).await
    }
}

/// `StartSleeping` / `StopSleeping`: positions zero and face 0 (capture 2026-10-02).
pub(crate) fn sleep_action(runtime_entity_id: u64, action: Action) -> PlayerAction {
    let origin = BlockCoordinates { x: 0, y: 0, z: 0 };
    PlayerAction { runtime_entity_id, action, position: origin.clone(), result_position: origin, face: 0 }
}

/// The server's message if `packet` is a refusal to let the player sleep.
pub(crate) fn bed_refusal(packet: &RawPacket) -> Option<String> {
    if packet.id != Text::ID {
        return None;
    }
    let message = ChatMessage::from_packet(packet.decode().ok()?)?;
    let plain = message.plain();
    let lower = plain.to_lowercase();
    REFUSALS.iter().any(|r| lower.contains(r)).then_some(plain)
}

mod tick;

pub(crate) use tick::Bed;

#[cfg(test)]
mod tests;
