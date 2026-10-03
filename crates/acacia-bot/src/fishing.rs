//! Fishing with a rod: cast, wait for a bite on the own hook (`state::Fishing`), reel in after a human
//! reaction time and report the catch. Packet sequence: docs/research/riding-fishing-elytra.md.

use std::time::Duration;

use acacia_client::proto::types::InputData;
use acacia_client::proto::RawPacket;

use crate::human::{FISH_REACTION, HOTBAR_SWITCH};
use crate::interact::{wire, SwingSource};
use crate::state::{Inventory, ItemStack};
use crate::{ActionError, Bot};

const FISHING_ROD: &str = "minecraft:fishing_rod";
/// How long the server has to spawn the hook after a cast.
const HOOK_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a reeled-in catch has to fly to the player and land in the inventory.
const CATCH_TIMEOUT: Duration = Duration::from_secs(3);

impl Bot {
    /// Fishes once: selects a rod from the hotbar if none is held, casts (unless the own hook is
    /// already out), waits up to `timeout` for a bite and reels in. Returns the item that landed in
    /// the inventory, or `None` when nothing bit, the hook was lost or the catch never arrived.
    /// Casts along the current rotation: [`Bot::look_at`] the water first.
    pub async fn fish(&mut self, timeout: Duration) -> Result<Option<ItemStack>, ActionError> {
        if self.hold_fishing_rod()? {
            let delay = self.human.between(HOTBAR_SWITCH);
            self.pause(delay).await?;
        }
        if self.state.fishing.hook.is_none() {
            self.cast().await?;
        }
        let bites = self.state.fishing.hook.as_ref().map_or(0, |h| h.bites);
        let bitten = |bot: &Bot, _: &RawPacket| match &bot.state.fishing.hook {
            None => Some(false),
            Some(hook) => (hook.bites > bites).then_some(true),
        };
        let bitten = match self.wait_until(timeout, bitten).await {
            Err(ActionError::Timeout) => false,
            other => other?,
        };
        tracing::debug!(bitten, hook = ?self.state.fishing.hook, "fishing wait over");
        if !bitten {
            if self.state.fishing.hook.is_some() {
                self.use_rod();
            }
            return Ok(None);
        }
        let reaction = self.human.between(FISH_REACTION);
        self.pause(reaction).await?;
        let before = self.state.inventory.main.clone();
        self.state.fishing.catch = None;
        self.use_rod();
        // BDS shows the catch only as a pickup (state::Fishing::catch); other servers update the slot.
        let caught = |bot: &Bot, _: &RawPacket| bot.state.fishing.catch.clone().or_else(|| gained(&before, &bot.state.inventory.main));
        match self.wait_until(CATCH_TIMEOUT, caught).await {
            Ok(item) => Ok(Some(item)),
            Err(ActionError::Timeout) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Selects a rod from the hotbar unless one is held; true if it switched.
    fn hold_fishing_rod(&mut self) -> Result<bool, ActionError> {
        let rod = self.state.items.id(FISHING_ROD);
        if rod.is_some_and(|id| self.state.inventory.held().network_id == id) {
            return Ok(false);
        }
        let hotbar = self.state.inventory.main.iter().take(usize::from(Inventory::HOTBAR_SLOTS));
        let slot = rod.and_then(|id| hotbar.clone().position(|s| s.network_id == id));
        match slot {
            Some(slot) => self.select_hotbar(slot as u8).map(|()| true),
            None => Err(ActionError::NotPossible("no fishing rod in the hotbar".into())),
        }
    }

    /// Casts or reels in, which vanilla sends identically: the use swing, `UseItem` ClickAir and
    /// `StartUsingItem` on the next input.
    fn use_rod(&mut self) {
        self.use_item_like_vanilla();
        self.queued_flags.push(InputData::StartUsingItem);
    }

    /// A right-click in the air with a rod or rocket as the capture shows it: `Animate` "useitem", then ClickAir.
    pub(crate) fn use_item_like_vanilla(&mut self) {
        self.swing_from(Some(SwingSource::UseItem));
        self.client.send(&wire::click_air(self.hand()));
    }

    async fn cast(&mut self) -> Result<(), ActionError> {
        self.use_rod();
        let hooked = |bot: &Bot, _: &RawPacket| bot.state.fishing.hook.is_some().then_some(());
        match self.wait_until(HOOK_TIMEOUT, hooked).await {
            Err(ActionError::Timeout) => Err(ActionError::Rejected("no fishing hook appeared".into())),
            other => other,
        }
    }
}

/// The first stack that grew between two snapshots of the same slots, as the amount added.
pub(crate) fn gained(before: &[ItemStack], after: &[ItemStack]) -> Option<ItemStack> {
    after.iter().zip(before).find_map(|(now, was)| {
        if now.is_empty() {
            return None;
        }
        let had = if was.network_id == now.network_id && !was.is_empty() { was.count } else { 0 };
        (now.count > had).then(|| ItemStack { count: now.count - had, ..now.clone() })
    })
}

#[cfg(test)]
mod tests {
    use acacia_client::proto::types::{
        TransactionTransactionData, TransactionUseItemClientPrediction, TransactionUseItemTriggerType as TriggerType,
    };

    use super::*;

    fn stack(network_id: i32, count: u16) -> ItemStack {
        ItemStack { network_id, count, ..ItemStack::default() }
    }

    #[test]
    fn gained_reports_new_and_grown_stacks() {
        let before = vec![stack(5, 3), ItemStack::default(), stack(7, 1)];
        assert_eq!(gained(&before, &before), None);
        assert_eq!(gained(&before, &[stack(5, 4), ItemStack::default(), stack(7, 1)]), Some(stack(5, 1)));
        assert_eq!(gained(&before, &[stack(5, 3), stack(9, 1), stack(7, 1)]), Some(stack(9, 1)));
        assert_eq!(gained(&before, &[stack(5, 2), ItemStack::default(), stack(8, 1)]), Some(stack(8, 1)), "replaced slot");
    }

    #[test]
    fn rod_and_rocket_clicks_match_the_capture() {
        let hand = wire::Hand { slot: 1, item: wire::to_wire(&stack(395, 1)), eye: [-0.42, 72.62, 3.44] };
        let TransactionTransactionData::ItemUse(u) = wire::click_air(hand).transaction.transaction_data else { panic!() };
        assert_eq!((u.trigger_type, u.client_prediction), (TriggerType::UnknownValue, TransactionUseItemClientPrediction::Failure));
        assert_eq!((u.face, u.hotbar_slot, u.click_pos.x), (255, 1, 0.0));
    }
}
