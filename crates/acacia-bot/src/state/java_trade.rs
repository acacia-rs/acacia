//! A Java merchant menu's offers as the bot's [`TradeWindow`], named in Bedrock's terms like the inventory mirror.

use acacia_java_client::Inventory as JavaInventory;
use acacia_java_proto::merchant::MerchantOffer;
use acacia_world::neutral::Item;

use super::{Stations, TradeItem, TradeOffer, TradeWindow};

/// Sets `stations.trade` from the open merchant menu, or clears it once none is open.
pub(crate) fn mirror_trade(stations: &mut Stations, java: &JavaInventory) {
    let window = java.window.as_ref().and_then(|w| Some((w, w.merchant.as_ref()?)));
    stations.trade = window.map(|(window, merchant)| TradeWindow {
        window_id: window.id,
        trader: 0,
        ids: [0, 0],
        display_name: window.title.clone().unwrap_or_default(),
        // Java's levels start at 1 (novice); Bedrock's tiers at 0. Java sends only the offers unlocked so far.
        tier: (merchant.level - 1).max(0),
        offers: merchant.offers.iter().map(offer).collect(),
    });
}

fn offer(o: &MerchantOffer) -> TradeOffer {
    TradeOffer {
        buy_a: item(o.cost_a.item, o.price_a()),
        buy_b: o.cost_b.as_ref().map(|b| item(b.item, b.count)),
        sell: o.result.as_ref().map_or_else(TradeItem::default, |r| item(r.item, r.count)),
        uses: o.uses,
        // Java's out-of-stock flag covers what Bedrock reads off uses reaching max uses.
        max_uses: if o.out_of_stock { o.uses } else { o.max_uses },
        tier: 0,
        recipe_network_id: 0,
    }
}

fn item(java_id: i32, count: i32) -> TradeItem {
    let (name, metadata) = u16::try_from(java_id).ok().and_then(Item::from_java_id).map_or((String::new(), 0), |i| {
        let (name, metadata) = i.bedrock();
        (name.to_owned(), metadata)
    });
    TradeItem { name, count: u16::try_from(count).unwrap_or(0), metadata }
}
