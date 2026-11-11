//! [`Bot::trade`] on Java, as the vanilla screen does it: picking the offer (`SelectTrade`) has the server move its
//! cost into the payment slots, and the result it then shows is taken. When every trade that payment covers is
//! wanted, one shift-click on the result makes them all; otherwise each trade is a click on the result and one into
//! the inventory. Nothing is predicted: prices, stock and the result are the server's.

use acacia_java_proto::merchant::MerchantOffer;
use acacia_java_proto::item::ItemStack as JavaStack;

use super::java_craft::RESULT_TIMEOUT;
use super::trade::OPEN_TIMEOUT;
use crate::bot::Backend;
use crate::human;
use crate::items::{ui, SlotRef};
use crate::{ActionError, Bot};

/// The merchant menu's slots: two payments, then the result.
const PAYMENT: [usize; 2] = [0, 1];
const RESULT: usize = 2;

impl Bot {
    pub(super) async fn java_trade(&mut self, trader: u64, offer: usize, times: u32) -> Result<u32, ActionError> {
        self.interact_entity(trader).await?;
        self.wait_until(OPEN_TIMEOUT, |bot: &Bot, _: &_| offers(bot).map(drop)).await?;
        self.human_pause(human::SCREEN_OPEN_LOOK).await?;
        let result = self.java_trades(offer, times).await;
        let back = self.take_back(&[ui::TRADE_INGREDIENT_1, ui::TRADE_INGREDIENT_2].map(SlotRef::Ui)).await;
        self.human_pause(human::SCREEN_LINGER).await?;
        self.close_container().await?;
        let done = result?;
        back.map(|()| done)
    }

    async fn java_trades(&mut self, offer: usize, times: u32) -> Result<u32, ActionError> {
        let mut done = 0;
        while done < times {
            let chosen = offers(self).and_then(|o| o.get(offer)).cloned();
            let chosen = chosen.ok_or_else(|| ActionError::NotPossible(format!("the trader has no offer {offer}")))?;
            if chosen.out_of_stock || chosen.uses >= chosen.max_uses {
                return sold_out(done, offer);
            }
            if done > 0 {
                self.click_pause().await?;
            }
            self.select_trade(offer)?;
            let shown = |bot: &Bot, _: &_| slot(bot, RESULT).map(drop);
            match self.wait_until(RESULT_TIMEOUT, shown).await {
                Err(ActionError::Timeout) if done > 0 => break,
                Err(ActionError::Timeout) => return Err(ActionError::NotPossible(format!("not enough to pay for offer {offer}"))),
                other => other?,
            }
            let covered = payable(self, &chosen);
            let batch = covered.min(times - done);
            self.click_pause().await?;
            done += if batch == covered { self.java_take_trades(batch).await? } else { self.java_take_one(RESULT).await.map(|()| 1)? };
        }
        Ok(done)
    }

    /// Shift-clicks the result, trading until the payment runs out: `expected` trades unless the inventory fills.
    async fn java_take_trades(&mut self, expected: u32) -> Result<u32, ActionError> {
        let (item, per_trade) = slot(self, RESULT).map_or((-1, 1), |s| (s.item, s.count.max(1) as u32));
        let before = holding(self, item);
        self.java_take_result(RESULT).await?;
        let emptied = |bot: &Bot, _: &_| slot(bot, RESULT).is_none().then_some(());
        let waited = self.wait_until(RESULT_TIMEOUT, emptied).await;
        let made = (holding(self, item).saturating_sub(before) / per_trade).min(expected);
        match waited {
            Err(ActionError::Timeout) if made == 0 => Err(ActionError::NotPossible("no room for the trade's result".into())),
            Err(ActionError::Timeout) => Ok(made),
            other => other.map(|()| made),
        }
    }

    fn select_trade(&mut self, offer: usize) -> Result<(), ActionError> {
        let Backend::Java(java) = &mut self.backend else { return Err(ActionError::Disconnected) };
        java.client.select_trade(i32::try_from(offer).unwrap_or(i32::MAX)).map_err(|_| ActionError::Disconnected)
    }
}

/// How many of Java item `item` the inventory holds.
fn holding(bot: &Bot, item: i32) -> u32 {
    bot.state.inventory.main.iter().filter(|s| !s.is_empty() && s.network_id == item).map(|s| u32::from(s.count)).sum()
}

fn sold_out(done: u32, offer: usize) -> Result<u32, ActionError> {
    if done > 0 { Ok(done) } else { Err(ActionError::NotPossible(format!("offer {offer} is sold out"))) }
}

/// Trades the payment slots cover, within the offer's stock.
fn payable(bot: &Bot, offer: &MerchantOffer) -> u32 {
    let count = |i: usize| slot(bot, PAYMENT[i]).map_or(0, |s| s.count);
    let a = count(0) / offer.price_a();
    let b = if offer.cost_b.is_some() { count(1) / offer.price_b().max(1) } else { i32::MAX };
    let stock = offer.max_uses - offer.uses;
    u32::try_from(a.min(b).min(stock)).unwrap_or(0)
}

fn offers(bot: &Bot) -> Option<&[MerchantOffer]> {
    Some(&bot.java()?.client().inventory.window.as_ref()?.merchant.as_ref()?.offers)
}

fn slot(bot: &Bot, i: usize) -> Option<&JavaStack> {
    bot.java()?.client().inventory.window.as_ref()?.slots.get(i)?.as_ref()
}
