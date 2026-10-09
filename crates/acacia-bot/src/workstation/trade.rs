//! Villager and wandering trader trades, shaped like the 2026-10-02 vanilla capture
//! (docs/research/workstations.md "Trades"): `UpdateTrade` opens the screen, the payment goes in as
//! whole stacks, each trade takes its result onto the cursor, and the cursor stack and payment
//! leftovers go back to the inventory before the close.

use std::time::Duration;

use acacia_client::proto::RawPacket;

use super::matching_slots;
use crate::human;
use crate::items::craft::{Craft, CraftAction};
use crate::items::{to_inventory_ops, ui, Op, SlotRef};
use crate::state::{GameState, ItemStack, TradeItem, TradeOffer};
use crate::{ActionError, Bot};

const OPEN_TIMEOUT: Duration = Duration::from_secs(3);
/// Assumed stack size of payments and results; the item registry does not carry it.
const MAX_STACK: u16 = 64;
const PAYMENT_SLOTS: [u8; 2] = [ui::TRADE_INGREDIENT_1, ui::TRADE_INGREDIENT_2];

impl Bot {
    /// Trades offer `offer` (index into `state().stations.trade`'s offers) `times` times with the
    /// tracked villager or wandering trader `trader` (runtime id; needs `Trackers::entities`).
    /// Returns how many trades went through (fewer only if payment or stock ran out after one).
    pub async fn trade(&mut self, trader: u64, offer: usize, times: u32) -> Result<u32, ActionError> {
        self.state.stations.trade = None;
        self.interact_entity(trader).await?;
        self.wait_until(OPEN_TIMEOUT, |bot: &Bot, _: &RawPacket| bot.state.stations.trade.is_some().then_some(())).await?;
        self.human_pause(human::SCREEN_OPEN_LOOK).await?;
        let mut sources = Vec::new();
        let result = self.trade_open(offer, times, &mut sources).await;
        let back = self.trade_leftovers(&sources).await;
        self.human_pause(human::SCREEN_LINGER).await?;
        self.close_container().await?;
        let done = result?;
        back.map(|()| done)
    }

    /// The trades; `sources` collects `(payment slot, inventory slot)` for every stack paid in.
    async fn trade_open(&mut self, offer: usize, times: u32, sources: &mut Vec<(SlotRef, SlotRef)>) -> Result<u32, ActionError> {
        let mut done = 0;
        let mut first = true;
        while done < times {
            let fill = match fill_ops(&self.state, offer) {
                Ok(fill) => fill,
                Err(ActionError::NotPossible(_)) if done > 0 => break,
                Err(e) => return Err(e),
            };
            if !fill.is_empty() {
                self.next_click(&mut first).await?;
                sources.extend(fill.iter().filter_map(|op| match *op {
                    Op::Transfer { from, to, .. } => Some((to, from)),
                    _ => None,
                }));
                self.item_stack_request(&fill).await?;
            }
            if !cursor_takes(&self.state, offer)? {
                self.next_click(&mut first).await?;
                self.cursor_to_inventory().await?;
            }
            self.next_click(&mut first).await?;
            let (craft, ops) = trade_plan(&self.state, offer)?;
            let uses = chosen(&self.state, offer)?.uses;
            self.craft_request(&craft, &ops).await?;
            count_use(&mut self.state, offer, uses);
            done += 1;
        }
        Ok(done)
    }

    /// One trade of offer `offer` on the open trading screen, as a player's click on the offer
    /// and then on its result: paid from the inventory, the result put into it.
    pub async fn trade_once(&mut self, offer: usize) -> Result<(), ActionError> {
        let fill = fill_ops(&self.state, offer)?;
        if !fill.is_empty() {
            self.item_stack_request(&fill).await?;
        }
        if !self.state.inventory.cursor().is_empty() {
            self.cursor_to_inventory().await?;
        }
        let (craft, ops) = trade_plan(&self.state, offer)?;
        let uses = chosen(&self.state, offer)?.uses;
        self.craft_request(&craft, &ops).await?;
        count_use(&mut self.state, offer, uses);
        self.cursor_to_inventory().await
    }

    async fn next_click(&mut self, first: &mut bool) -> Result<(), ActionError> {
        if std::mem::take(first) {
            return Ok(());
        }
        self.click_pause().await
    }

    async fn cursor_to_inventory(&mut self) -> Result<(), ActionError> {
        let held = self.state.inventory.cursor().clone();
        let ops = to_inventory_ops(&self.state, SlotRef::Cursor, &held)?;
        self.item_stack_request(&ops).await
    }

    /// As vanilla: the traded items on the cursor into the inventory, then each payment's
    /// leftovers back to the slot they came from.
    async fn trade_leftovers(&mut self, sources: &[(SlotRef, SlotRef)]) -> Result<(), ActionError> {
        if !self.state.inventory.cursor().is_empty() {
            self.click_pause().await?;
            self.cursor_to_inventory().await?;
        }
        for slot in PAYMENT_SLOTS.map(SlotRef::Ui) {
            let Some(left) = slot.stack(&self.state).filter(|s| !s.is_empty()).cloned() else { continue };
            self.click_pause().await?;
            match return_slot(&self.state, slot, &left, sources) {
                Some(to) => self.move_item(slot, to, left.count as u8).await?,
                None => {
                    self.quick_move(slot).await?;
                }
            }
        }
        Ok(())
    }
}

fn chosen(state: &GameState, offer: usize) -> Result<&TradeOffer, ActionError> {
    let window = state.stations.trade.as_ref().ok_or_else(|| ActionError::NotPossible("no trade screen is open".into()))?;
    let chosen = window.offers.get(offer).ok_or_else(|| ActionError::NotPossible(format!("the trader has {} offers", window.offers.len())))?;
    if chosen.is_disabled() {
        return Err(ActionError::NotPossible(format!("offer {offer} is sold out")));
    }
    if chosen.tier > window.tier {
        return Err(ActionError::NotPossible(format!("offer {offer} needs trader level {}", chosen.tier)));
    }
    Ok(chosen)
}

/// `Place`s of whole inventory stacks into UI slots 4/5 until each holds the offer's price, as the
/// trade UI's auto-fill does (one request).
pub(crate) fn fill_ops(state: &GameState, offer: usize) -> Result<Vec<Op>, ActionError> {
    let chosen = chosen(state, offer)?;
    let mut ops = Vec::new();
    let payments = [(Some(&chosen.buy_a), ui::TRADE_INGREDIENT_1), (chosen.buy_b.as_ref(), ui::TRADE_INGREDIENT_2)];
    for (item, slot) in payments {
        let Some(item) = item else { continue };
        let mut have = SlotRef::Ui(slot).stack(state).filter(|s| state.items.name(s.network_id) == Some(item.name.as_str())).map_or(0, |s| s.count);
        for (from, available) in matching_slots(state, |name, meta| pays(item, name, meta)) {
            if have >= item.count {
                break;
            }
            let n = available.min(MAX_STACK.saturating_sub(have));
            ops.push(Op::Transfer { from, to: SlotRef::Ui(slot), count: n as u8 });
            have += n;
        }
        if have < item.count {
            return Err(ActionError::NotPossible(format!("not enough {} to pay", item.name)));
        }
    }
    Ok(ops)
}

/// The trade itself, once the payment sits in UI slots 4/5: the result is taken onto the cursor.
pub(crate) fn trade_plan(state: &GameState, offer: usize) -> Result<(Craft, Vec<Op>), ActionError> {
    let chosen = chosen(state, offer)?;
    let result = sold(state, chosen)?;
    let craft = Craft::new(CraftAction::Recipe { network_id: chosen.recipe_network_id, times: 1 }, vec![(chosen.sell.name.clone(), result.clone())]).with_results_action();
    let mut ops = vec![Op::Consume { from: SlotRef::Ui(ui::TRADE_INGREDIENT_1), count: chosen.buy_a.count as u8 }];
    if let Some(b) = &chosen.buy_b {
        ops.push(Op::Consume { from: SlotRef::Ui(ui::TRADE_INGREDIENT_2), count: b.count as u8 });
    }
    ops.push(Op::Transfer { from: SlotRef::CREATED_OUTPUT, to: SlotRef::Cursor, count: result.count as u8 });
    Ok((craft, ops))
}

/// Counts a trade into the offer's `uses` (sold out at `max_uses`) unless an `UpdateTrade` already
/// changed it from `before`.
fn count_use(state: &mut GameState, offer: usize, before: i32) {
    if let Some(chosen) = state.stations.trade.as_mut().and_then(|t| t.offers.get_mut(offer))
        && chosen.uses == before
    {
        chosen.uses += 1;
    }
}

fn sold(state: &GameState, offer: &TradeOffer) -> Result<ItemStack, ActionError> {
    let sell = &offer.sell;
    let network_id = state.items.id(&sell.name).ok_or_else(|| ActionError::NotPossible(format!("{} is not in the registry", sell.name)))?;
    Ok(ItemStack { network_id, count: sell.count, metadata: sell.metadata as u32, ..ItemStack::default() })
}

/// Whether the next result can join the cursor stack (else the cursor is emptied first).
fn cursor_takes(state: &GameState, offer: usize) -> Result<bool, ActionError> {
    let cursor = state.inventory.cursor();
    if cursor.is_empty() {
        return Ok(true);
    }
    let result = sold(state, chosen(state, offer)?)?;
    let same = cursor.network_id == result.network_id && cursor.metadata == result.metadata && cursor.nbt == result.nbt;
    Ok(same && cursor.count + result.count <= MAX_STACK)
}

/// The inventory slot a payment stack came from, if it can take `left` back.
fn return_slot(state: &GameState, slot: SlotRef, left: &ItemStack, sources: &[(SlotRef, SlotRef)]) -> Option<SlotRef> {
    sources.iter().filter(|(paid, _)| *paid == slot).map(|&(_, from)| from).find(|from| {
        from.stack(state).is_some_and(|s| s.is_empty() || (s.network_id == left.network_id && s.metadata == left.metadata && s.count + left.count <= MAX_STACK))
    })
}

/// BDS writes payment `Damage` 32767 (any aux value) for most items; 0 is a real value (a water
/// bottle is potion 0, an awkward potion would not pay).
fn pays(item: &TradeItem, name: &str, metadata: u32) -> bool {
    name == item.name && (item.metadata == i16::MAX || metadata == item.metadata as u32)
}
