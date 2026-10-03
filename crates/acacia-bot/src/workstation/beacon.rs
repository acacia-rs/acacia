//! Beacon, matching the 2026-10-02 vanilla capture: the payment goes into UI slot 27 with one
//! click; confirming sends `BeaconPayment` followed by a `Destroy` (not `Consume`) of the payment,
//! which Allay and Nukkit-MOT also require (dragonfly removes the payment itself and skips that
//! Destroy; Geyser reads only the first action).

use acacia_client::proto::types::{ItemStackRequestActionsItem as Action, WindowType};
use acacia_physics::BlockPos;

use super::occupied;
use crate::human;
use crate::items::{beacon_payment, ui, Op, SlotRef};
use crate::state::GameState;
use crate::{ActionError, Bot};

const PAYMENTS: [&str; 5] = ["minecraft:iron_ingot", "minecraft:gold_ingot", "minecraft:emerald", "minecraft:diamond", "minecraft:netherite_ingot"];

/// A beacon power, by its Bedrock effect id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BeaconEffect {
    Speed = 1,
    Haste = 3,
    Strength = 5,
    JumpBoost = 8,
    Regeneration = 10,
    Resistance = 11,
}

impl Bot {
    /// Sets the beacon at `pos` to `primary`, plus `secondary` on a full pyramid (regeneration, or
    /// the primary again for level II), paying one item from `payment` (iron, gold, emerald,
    /// diamond or netherite ingot).
    pub async fn beacon(&mut self, pos: BlockPos, primary: BeaconEffect, secondary: Option<BeaconEffect>, payment: SlotRef) -> Result<(), ActionError> {
        let item = occupied(&self.state, payment)?;
        let name = self.state.items.name(item.network_id).unwrap_or_default();
        if !PAYMENTS.contains(&name) {
            return Err(ActionError::NotPossible(format!("{name} cannot pay a beacon")));
        }
        self.open_station(pos, &[WindowType::Beacon]).await?;
        let slot = SlotRef::Ui(ui::BEACON_PAYMENT);
        let result = self.beacon_open(payment, primary, secondary).await;
        let back = self.take_back(&[slot]).await;
        self.human_pause(human::SCREEN_LINGER).await?;
        self.close_container().await?;
        result.and(back)
    }

    async fn beacon_open(&mut self, payment: SlotRef, primary: BeaconEffect, secondary: Option<BeaconEffect>) -> Result<(), ActionError> {
        self.human_pause(human::SCREEN_OPEN_LOOK).await?;
        self.move_item(payment, SlotRef::Ui(ui::BEACON_PAYMENT), 1).await?;
        self.human_pause(human::BEACON_CONFIRM).await?;
        let (head, ops) = beacon_plan(&self.state, primary, secondary)?;
        self.headed_request(head, &ops).await
    }
}

/// The confirm request: `BeaconPayment`, then the payment destroyed.
pub(crate) fn beacon_plan(state: &GameState, primary: BeaconEffect, secondary: Option<BeaconEffect>) -> Result<(Vec<Action>, Vec<Op>), ActionError> {
    if secondary.is_some_and(|s| s != primary && s != BeaconEffect::Regeneration) {
        return Err(ActionError::NotPossible("the secondary power must be regeneration or the primary one".into()));
    }
    let slot = SlotRef::Ui(ui::BEACON_PAYMENT);
    occupied(state, slot)?;
    let head = vec![beacon_payment(primary as i32, secondary.map_or(0, |s| s as i32))];
    Ok((head, vec![Op::Destroy { from: slot, count: 1 }]))
}
