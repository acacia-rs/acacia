//! Anvil (`CraftRecipeOptional`, the new name as the request's filter string). No server checks
//! its recipe id. BDS computes the result itself and rejects a request whose `Consume`s differ from
//! what it uses up (status 5), so the plan predicts the craft exactly (`anvil_cost.rs`).

use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::types::WindowType;
use acacia_physics::BlockPos;

use super::anvil_cost::anvil_outcome;
use super::{named, occupied};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::{ActionError, Bot};

impl Bot {
    /// Repairs, combines and/or renames at the anvil at `pos`: `input` goes into the first slot,
    /// `material` (if any) into the second, and the result into the inventory. `NotPossible` when
    /// the server would make nothing: no change, too expensive (40+ levels) or above the bot's level.
    pub async fn anvil(&mut self, pos: BlockPos, input: SlotRef, material: Option<SlotRef>, rename: Option<&str>) -> Result<(), ActionError> {
        if material.is_none() && rename.is_none() {
            return Err(ActionError::NotPossible("an anvil needs a material or a new name".into()));
        }
        let slots = [SlotRef::Ui(ui::ANVIL_INPUT), SlotRef::Ui(ui::ANVIL_MATERIAL)];
        self.workstation_craft(pos, WindowType::Anvil, &[Some(input), material], &slots, |state| anvil_plan(state, rename)).await
    }

    /// Opens the station, moves each `inputs[i]` stack into `slots[i]`, sends the craft `plan`
    /// builds, moves leftovers back and closes.
    pub(super) async fn workstation_craft(
        &mut self,
        pos: BlockPos,
        kind: WindowType,
        inputs: &[Option<SlotRef>],
        slots: &[SlotRef],
        plan: impl Fn(&GameState) -> Result<(Craft, Vec<Op>), ActionError>,
    ) -> Result<(), ActionError> {
        for input in inputs.iter().flatten() {
            occupied(&self.state, *input)?;
        }
        self.open_station_and_look(pos, &[kind]).await?;
        let mut result = Ok(());
        for (input, &slot) in inputs.iter().zip(slots) {
            let Some(input) = *input else { continue };
            let count = input.stack(&self.state).map_or(0, |s| s.count) as u8;
            result = self.put(input, slot, count).await;
            if result.is_err() {
                break;
            }
        }
        if result.is_ok() {
            result = match plan(&self.state) {
                Ok(craft) => self.craft_click(craft).await,
                Err(e) => Err(e),
            };
        }
        let back = self.take_back(slots).await;
        self.close_station().await?;
        result.and(back)
    }
}

pub(crate) fn anvil_plan(state: &GameState, rename: Option<&str>) -> Result<(Craft, Vec<Op>), ActionError> {
    let (input, material) = (SlotRef::Ui(ui::ANVIL_INPUT), SlotRef::Ui(ui::ANVIL_MATERIAL));
    let item = occupied(state, input)?;
    let outcome = anvil_outcome(state, item, occupied(state, material).ok(), rename)?;
    let mut craft = Craft::new(CraftAction::Optional { network_id: 0, filter_index: 0 }, vec![named(state, &outcome.result)?]);
    if let Some(name) = rename {
        craft = craft.renaming(name);
    }
    // Vanilla consumes the material before the input (2026-10-02 capture).
    let mut ops = Vec::new();
    if outcome.material_used > 0 {
        ops.push(Op::Consume { from: material, count: outcome.material_used as u8 });
    }
    ops.push(Op::Consume { from: input, count: item.count as u8 });
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &outcome.result)?);
    Ok((craft, ops))
}

pub(super) fn nbt_int(stack: &ItemStack, key: &str) -> i32 {
    match stack.nbt.as_ref().and_then(|n| n.value.get(key)) {
        Some(Value::Int(v)) => *v,
        _ => 0,
    }
}

/// Sets `key` in the stack's root compound, keeping keys in byte order like the client.
pub(super) fn set_nbt(stack: &mut ItemStack, key: &str, value: Value) {
    let root = stack.nbt.get_or_insert_with(|| Nbt { name: String::new(), value: Value::Compound(Vec::new()) });
    let Value::Compound(entries) = &mut root.value else { return };
    entries.retain(|(k, _)| k != key);
    entries.push((key.into(), value));
    entries.sort_by(|a, b| a.0.cmp(&b.0));
}
