//! Enchanting table: the item goes into UI slot 14 and lapis into 15; the server then sends
//! `PlayerEnchantOptions`. Enchanting is a `CraftRecipe` with the option's recipe network id that
//! consumes the item and index + 1 lapis; the enchanted item is placed back into slot 14 and
//! moved out by a separate click.

use std::time::Duration;

use acacia_client::proto::nbt::{List, Nbt, Value};
use acacia_client::proto::types::{GameMode, WindowType};
use acacia_client::proto::RawPacket;
use acacia_physics::BlockPos;

use super::{named, occupied};
use crate::items::craft::{Craft, CraftAction};
use crate::items::{ui, Op, SlotRef};
use crate::state::{GameState, ItemStack};
use crate::{ActionError, Bot};

const OPTIONS_TIMEOUT: Duration = Duration::from_secs(3);
const LAPIS: &str = "minecraft:lapis_lazuli";
const COMPOUND_TAG: u8 = 10;

impl Bot {
    /// Enchants the item in `item` at the table at `pos` with option `option` (0-2, cheapest
    /// first) and puts it back into the inventory. Needs the option's level requirement and
    /// `option + 1` lapis lazuli.
    pub async fn enchant(&mut self, pos: BlockPos, item: SlotRef, option: usize) -> Result<(), ActionError> {
        occupied(&self.state, item)?;
        self.open_station_and_look(pos, &[WindowType::Enchantment]).await?;
        let result = self.enchant_open(item, option).await;
        let back = self.take_back(&[SlotRef::Ui(ui::ENCHANTING_INPUT), SlotRef::Ui(ui::ENCHANTING_LAPIS)]).await;
        self.close_station().await?;
        result.and(back)
    }

    async fn enchant_open(&mut self, item: SlotRef, option: usize) -> Result<(), ActionError> {
        self.state.stations.enchant_options.clear();
        self.put(item, SlotRef::Ui(ui::ENCHANTING_INPUT), 1).await?;
        let lapis: u16 = super::matching_slots(&self.state, |name, _| name == LAPIS).iter().map(|(_, n)| *n).sum();
        self.put_matching(|name, _| name == LAPIS, lapis.min(64), SlotRef::Ui(ui::ENCHANTING_LAPIS)).await?;
        let offered = |bot: &Bot, _: &RawPacket| (!bot.state.stations.enchant_options.is_empty()).then_some(());
        self.wait_until(OPTIONS_TIMEOUT, offered).await?;
        let plan = enchant_plan(&self.state, option)?;
        let level = self.state.player.xp_level;
        self.craft_click(plan).await?;
        // BDS takes option + 1 levels (not the cost); its attribute update may come after the response.
        if self.state.player.xp_level == level && self.state.player.game_mode != GameMode::Creative {
            self.state.player.xp_level -= option as i32 + 1;
        }
        Ok(())
    }
}

/// `CraftRecipe` for enchanting option `option`, consuming the item and the lapis, with the
/// result placed back into the input slot.
pub(crate) fn enchant_plan(state: &GameState, option: usize) -> Result<(Craft, Vec<Op>), ActionError> {
    let options = &state.stations.enchant_options;
    let chosen = options.get(option).ok_or_else(|| ActionError::NotPossible(format!("only {} enchanting options", options.len())))?;
    if state.player.xp_level < i32::from(chosen.cost) {
        return Err(ActionError::NotPossible(format!("option {option} needs level {}", chosen.cost)));
    }
    let (input, lapis) = (SlotRef::Ui(ui::ENCHANTING_INPUT), SlotRef::Ui(ui::ENCHANTING_LAPIS));
    let item = occupied(state, input)?;
    let lapis_cost = option as u8 + 1;
    if occupied(state, lapis)?.count < u16::from(lapis_cost) {
        return Err(ActionError::NotPossible(format!("option {option} needs {lapis_cost} lapis")));
    }
    // The server sends no slot update after the craft, so the prediction is all the bot will see.
    let result = with_enchants(item, &chosen.enchants);
    let craft = Craft::new(CraftAction::Recipe { network_id: chosen.recipe_network_id, times: 1 }, vec![named(state, &result)?]).with_results_action();
    // Vanilla order (2026-10-02 capture): the lapis is consumed after the result is placed.
    let ops = vec![
        Op::Consume { from: input, count: 1 },
        Op::Transfer { from: SlotRef::CREATED_OUTPUT, to: input, count: 1 },
        Op::Consume { from: lapis, count: lapis_cost },
    ];
    Ok((craft, ops))
}

/// `item` with `enchants` (id, level) appended to its `ench` list, as Bedrock item NBT holds them.
pub(crate) fn with_enchants(item: &ItemStack, enchants: &[(u8, u8)]) -> ItemStack {
    let mut result = item.clone();
    let root = result.nbt.get_or_insert_with(|| Nbt { name: String::new(), value: Value::Compound(Vec::new()) });
    let Value::Compound(entries) = &mut root.value else { return result };
    let added = enchants.iter().map(|&(id, lvl)| Value::Compound(vec![("id".into(), Value::Short(i16::from(id))), ("lvl".into(), Value::Short(i16::from(lvl)))]));
    match entries.iter_mut().find(|(k, _)| k == "ench") {
        Some((_, Value::List(list))) => list.items.extend(added),
        _ => {
            entries.push(("ench".into(), Value::List(List { tag: COMPOUND_TAG, items: added.collect() })));
            entries.sort_by(|a, b| a.0.cmp(&b.0));
        }
    }
    result
}
