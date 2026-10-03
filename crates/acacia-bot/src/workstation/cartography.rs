//! Cartography table and making maps, shaped like the 2026-10-02 vanilla capture
//! (docs/research/workstations.md "Cartography"). Every craft is `CraftRecipeOptional` with one
//! filter string (the new name, or `""`) and cause CartographyText, and no
//! `CraftResultsDeprecated`; a rename takes one craft per map.

use std::time::Duration;

use acacia_client::proto::nbt::Value;
use acacia_client::proto::types::WindowType;
use acacia_client::proto::RawPacket;
use acacia_physics::BlockPos;

use super::anvil::set_nbt;
use super::{named, occupied};
use crate::human;
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, Inventory, ItemStack};
use crate::{ActionError, Bot};

const FILLED_MAP: &str = "minecraft:filled_map";
const EMPTY_MAP: &str = "minecraft:empty_map";
const MAP_TIMEOUT: Duration = Duration::from_secs(2);

/// The cartography multi recipe (`CraftingData` UUID) the additional item selects, and the maps
/// one craft makes. Only cloning was captured; zooming and locking are by analogy.
fn operation(additional: &str) -> Option<(&'static str, u16)> {
    match additional {
        EMPTY_MAP => Some(("442d85ed-8272-4543-a6f1-418f90ded05d", 2)),
        "minecraft:paper" => Some(("8b36268c-1829-483c-a0f1-993b7156a8f2", 1)),
        "minecraft:glass_pane" => Some(("602234e4-cac1-4353-8bb7-b1ebff70024b", 1)),
        _ => None,
    }
}

impl Bot {
    /// At the cartography table at `pos`: puts the filled-map stack from `map` in, plus the stack
    /// from `additional` (an empty map clones, paper zooms out, a glass pane locks), optionally
    /// renames, and crafts up to `times` times (one map per rename craft; a clone makes two).
    /// Returns how many crafts went through.
    pub async fn cartography(&mut self, pos: BlockPos, map: SlotRef, additional: Option<SlotRef>, rename: Option<&str>, times: u32) -> Result<u32, ActionError> {
        if item_name(&self.state, map)? != FILLED_MAP {
            return Err(ActionError::NotPossible(format!("{map:?} holds no filled map")));
        }
        let usable = match additional {
            Some(slot) => operation(item_name(&self.state, slot)?).is_some(),
            None => rename.is_some(),
        };
        if !usable {
            return Err(ActionError::NotPossible("a cartography table needs an empty map, paper, a glass pane or a new name".into()));
        }
        self.open_station(pos, &[WindowType::Cartography]).await?;
        let result = self.cartography_open(map, additional, rename, times).await;
        let back = self.take_back(&[SlotRef::Ui(ui::CARTOGRAPHY_INPUT), SlotRef::Ui(ui::CARTOGRAPHY_ADDITIONAL)]).await;
        self.human_pause(human::SCREEN_LINGER).await?;
        self.close_container().await?;
        let done = result?;
        back.map(|()| done)
    }

    async fn cartography_open(&mut self, map: SlotRef, additional: Option<SlotRef>, rename: Option<&str>, times: u32) -> Result<u32, ActionError> {
        self.human_pause(human::SCREEN_OPEN_LOOK).await?;
        self.move_item(map, SlotRef::Ui(ui::CARTOGRAPHY_INPUT), whole(&self.state, map)).await?;
        if let Some(slot) = additional {
            self.put(slot, SlotRef::Ui(ui::CARTOGRAPHY_ADDITIONAL), whole(&self.state, slot)).await?;
        }
        if let Some(name) = rename {
            let delay = self.human.typing(name.chars().count());
            self.pause(delay).await?;
        }
        let mut done = 0;
        while done < times {
            let plan = match cartography_plan(&self.state, rename) {
                Ok(plan) => plan,
                Err(ActionError::NotPossible(_)) if done > 0 => break,
                Err(e) => return Err(e),
            };
            self.craft_click(plan).await?;
            done += 1;
        }
        Ok(done)
    }

    /// Uses the held empty map (`UseItem` ClickAir, no swing, as vanilla) and waits for the filled
    /// map the server makes; returns its slot. Vanilla then uploads its own render of the area
    /// (`MapInfoRequest` with 128×128 pixels); the bot cannot render terrain and sends none.
    pub async fn make_map(&mut self) -> Result<SlotRef, ActionError> {
        if self.state.items.name(self.state.inventory.held().network_id) != Some(EMPTY_MAP) {
            return Err(ActionError::NotPossible("no empty map held".into()));
        }
        let before = filled_maps(&self.state);
        self.use_item();
        let made = |bot: &Bot, _: &RawPacket| filled_maps(&bot.state).into_iter().find(|m| !before.contains(m)).map(|(slot, _)| slot);
        self.wait_until(MAP_TIMEOUT, made).await
    }
}

/// The craft for what sits in the input (12) and additional (13) slots: recipe 0 for a rename
/// alone, else the multi recipe the additional item selects.
pub(crate) fn cartography_plan(state: &GameState, rename: Option<&str>) -> Result<(Craft, Vec<Op>), ActionError> {
    let (input, additional) = (SlotRef::Ui(ui::CARTOGRAPHY_INPUT), SlotRef::Ui(ui::CARTOGRAPHY_ADDITIONAL));
    let map = occupied(state, input)?;
    let mut result = ItemStack { count: 1, ..map.clone() };
    let mut ops = vec![Op::Consume { from: input, count: 1 }];
    let mut recipe = 0;
    if occupied(state, additional).is_ok() {
        let (uuid, made) = operation(item_name(state, additional)?).ok_or_else(|| ActionError::NotPossible("the cartography table cannot use the additional item".into()))?;
        recipe = state.recipes.book().multi_recipe(uuid).ok_or_else(|| ActionError::NotPossible("the server sent no cartography recipes".into()))?;
        result.count = made;
        ops.push(Op::Consume { from: additional, count: 1 });
    } else if rename.is_none() {
        return Err(ActionError::NotPossible("nothing to craft".into()));
    }
    if let Some(name) = rename {
        result.custom_name = Some(name.to_owned());
        set_nbt(&mut result, "display", Value::Compound(vec![("Name".into(), Value::String(name.to_owned()))]));
    }
    let action = CraftAction::Optional { network_id: recipe, filter_index: 0 };
    let craft = Craft::new(action, vec![named(state, &result)?]).cartography_name(rename.unwrap_or(""));
    ops.extend(to_inventory_ops(state, SlotRef::CREATED_OUTPUT, &result)?);
    Ok((craft, ops))
}

fn item_name(state: &GameState, slot: SlotRef) -> Result<&str, ActionError> {
    let stack = occupied(state, slot)?;
    Ok(state.items.name(stack.network_id).unwrap_or_default())
}

fn whole(state: &GameState, slot: SlotRef) -> u8 {
    slot.stack(state).map_or(0, |s| u8::try_from(s.count).unwrap_or(u8::MAX))
}

/// `(slot, stack id)` of every filled map in the main inventory.
fn filled_maps(state: &GameState) -> Vec<(SlotRef, Option<i32>)> {
    (0..Inventory::MAIN_SLOTS as u8)
        .map(SlotRef::Main)
        .filter_map(|slot| {
            let stack = slot.stack(state).filter(|s| state.items.name(s.network_id) == Some(FILLED_MAP))?;
            Some((slot, stack.stack_network_id))
        })
        .collect()
}
