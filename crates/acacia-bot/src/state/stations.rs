//! What the open workstation offers: enchanting options (`PlayerEnchantOptions`) and trades
//! (`UpdateTrade`). Both are dropped when the window closes.

use acacia_client::proto::nbt::Value;
use acacia_client::proto::packets::{ContainerClose, PlayerEnchantOptions, UpdateTrade};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// One of the enchanting table's three options.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantOption {
    /// Levels the player needs; enchanting costs index + 1 levels and as much lapis.
    pub cost: u8,
    /// `(enchantment id, level)` shown as the hint.
    pub enchants: Vec<(u8, u8)>,
    /// The galactic-alphabet label.
    pub name: String,
    /// Recipe network id the `CraftRecipe` action refers to.
    pub recipe_network_id: u32,
}

/// One item of a trade offer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TradeItem {
    /// Identifier, `minecraft:emerald`.
    pub name: String,
    pub count: u16,
    pub metadata: i16,
}

/// One villager offer from the `Recipes` list of `UpdateTrade`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TradeOffer {
    pub buy_a: TradeItem,
    pub buy_b: Option<TradeItem>,
    pub sell: TradeItem,
    pub uses: i32,
    pub max_uses: i32,
    /// Level the trader must reach for the offer (0 = novice).
    pub tier: i32,
    /// Recipe network id the `CraftRecipe` action refers to (`netId`).
    pub recipe_network_id: u32,
}

impl TradeOffer {
    /// Sold out, BDS's rule (`maxUses >= 0 && uses >= maxUses`, else status 9): 0 max uses is sold out.
    pub fn is_disabled(&self) -> bool {
        self.max_uses >= 0 && self.uses >= self.max_uses
    }
}

/// The open trading screen.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeWindow {
    pub window_id: i32,
    /// Unique id of the villager or wandering trader.
    pub trader: i64,
    pub display_name: String,
    /// The trader's current level (0 = novice).
    pub tier: i32,
    pub offers: Vec<TradeOffer>,
}

#[derive(Debug, Default)]
pub struct Stations {
    pub enchant_options: Vec<EnchantOption>,
    pub trade: Option<TradeWindow>,
}

impl Stations {
    pub const PACKETS: &'static [u32] = &[PlayerEnchantOptions::ID, UpdateTrade::ID, ContainerClose::ID];

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            PlayerEnchantOptions::ID => {
                let p: PlayerEnchantOptions = packet.decode()?;
                self.enchant_options = p.options.into_iter().map(enchant_option).collect();
            }
            UpdateTrade::ID => {
                let p: UpdateTrade = packet.decode()?;
                let offers = match p.offers.value.get("Recipes") {
                    Some(Value::List(list)) => list.items.iter().map(trade_offer).collect(),
                    _ => Vec::new(),
                };
                self.trade = Some(TradeWindow {
                    window_id: p.window_id.to_raw() as i32,
                    trader: p.entity_unique_id as i64,
                    display_name: p.display_name,
                    tier: p.trade_tier as i32,
                    offers,
                });
            }
            ContainerClose::ID => {
                packet.decode::<ContainerClose>()?;
                self.enchant_options.clear();
                self.trade = None;
            }
            _ => {}
        }
        Ok(())
    }
}

fn enchant_option(o: acacia_client::proto::types::EnchantOption) -> EnchantOption {
    let enchants = o.equip_enchants.iter().chain(&o.held_enchants).chain(&o.self_enchants).map(|e| (e.id, e.level)).collect();
    EnchantOption { cost: o.cost, enchants, name: o.name, recipe_network_id: o.option_id }
}

fn trade_offer(offer: &Value) -> TradeOffer {
    let int = |key: &str| match offer.get(key) {
        Some(Value::Int(v)) => *v,
        Some(Value::Byte(v)) => i32::from(*v),
        Some(Value::Short(v)) => i32::from(*v),
        _ => 0,
    };
    // buyCountA/B carry the price after demand and discounts; the item counts are the base price.
    let payment = |item: &str, count: &str| {
        let mut payment = offer.get(item).map(trade_item).filter(|i| !i.name.is_empty() && i.name != "minecraft:air" && i.count > 0)?;
        if int(count) > 0 {
            payment.count = int(count) as u16;
        }
        Some(payment)
    };
    TradeOffer {
        buy_a: payment("buyA", "buyCountA").unwrap_or_default(),
        buy_b: payment("buyB", "buyCountB"),
        sell: offer.get("sell").map(trade_item).unwrap_or_default(),
        uses: int("uses"),
        max_uses: int("maxUses"),
        tier: int("tier"),
        recipe_network_id: int("netId") as u32,
    }
}

fn trade_item(item: &Value) -> TradeItem {
    let name = match item.get("Name") {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    let count = match item.get("Count") {
        Some(Value::Byte(c)) => *c as u8 as u16,
        Some(Value::Short(c)) => *c as u16,
        Some(Value::Int(c)) => *c as u16,
        _ => 0,
    };
    let metadata = match item.get("Damage") {
        Some(Value::Short(d)) => *d,
        _ => 0,
    };
    TradeItem { name, count, metadata }
}

#[cfg(test)]
mod tests;
