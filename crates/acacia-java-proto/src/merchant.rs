//! `ClientboundMerchantOffersPacket`: a villager's or wandering trader's offers, sent after its menu opens.

use crate::item::{Components, ItemStack};
use crate::{Error, VarInt, Wire, read_varint, write_varint};

/// Vanilla clamps a modified price to the cost item's stack size; every vanilla cost item stacks to 64.
const COST_STACK: i32 = 64;

/// A price: `count` of `item`, whose components must match `components` exactly.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemCost {
    pub item: i32,
    pub count: i32,
    pub components: Components,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MerchantOffer {
    pub cost_a: ItemCost,
    pub result: Option<ItemStack>,
    pub cost_b: Option<ItemCost>,
    pub out_of_stock: bool,
    pub uses: i32,
    pub max_uses: i32,
    pub xp: i32,
    pub special_price: i32,
    pub price_multiplier: f32,
    pub demand: i32,
}

impl MerchantOffer {
    /// `cost_a`'s count after demand and reputation (`MerchantOffer.getModifiedCostCount`).
    pub fn price_a(&self) -> i32 {
        let base = self.cost_a.count;
        let demand = ((base * self.demand) as f32 * self.price_multiplier).floor().max(0.0) as i32;
        (base + demand + self.special_price).clamp(1, COST_STACK)
    }

    pub fn price_b(&self) -> i32 {
        self.cost_b.as_ref().map_or(0, |b| b.count)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MerchantOffers {
    pub container: i32,
    pub offers: Vec<MerchantOffer>,
    pub level: i32,
    pub xp: i32,
    pub show_progress: bool,
    pub can_restock: bool,
}

impl Wire for ItemCost {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { item: read_varint(input)?, count: read_varint(input)?, components: Components::read_exact(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.item, out);
        write_varint(self.count, out);
        self.components.write_exact(out);
    }
}

impl Wire for MerchantOffer {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            cost_a: ItemCost::read(input)?,
            result: Option::<ItemStack>::read(input)?,
            cost_b: if bool::read(input)? { Some(ItemCost::read(input)?) } else { None },
            out_of_stock: bool::read(input)?,
            uses: i32::read(input)?,
            max_uses: i32::read(input)?,
            xp: i32::read(input)?,
            special_price: i32::read(input)?,
            price_multiplier: f32::read(input)?,
            demand: i32::read(input)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.cost_a.write(out);
        self.result.write(out);
        self.cost_b.is_some().write(out);
        if let Some(b) = &self.cost_b {
            b.write(out);
        }
        self.out_of_stock.write(out);
        [self.uses, self.max_uses, self.xp, self.special_price].iter().for_each(|v| v.write(out));
        self.price_multiplier.write(out);
        self.demand.write(out);
    }
}

impl Wire for MerchantOffers {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let container = read_varint(input)?;
        let offers = (0..crate::wire::read_len(input)?).map(|_| MerchantOffer::read(input)).collect::<Result<_, _>>()?;
        Ok(Self { container, offers, level: read_varint(input)?, xp: read_varint(input)?, show_progress: bool::read(input)?, can_restock: bool::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.container).write(out);
        write_varint(self.offers.len() as i32, out);
        self.offers.iter().for_each(|o| o.write(out));
        VarInt(self.level).write(out);
        VarInt(self.xp).write(out);
        self.show_progress.write(out);
        self.can_restock.write(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_round_trip_and_price_follows_demand() {
        let emeralds = ItemCost { item: 900, count: 10, components: Components::default() };
        let offer = MerchantOffer {
            cost_a: emeralds.clone(),
            result: Some(ItemStack::new(812, 1)),
            cost_b: Some(ItemCost { count: 1, ..emeralds }),
            uses: 2,
            max_uses: 12,
            demand: 3,
            price_multiplier: 0.05,
            special_price: -2,
            ..MerchantOffer::default()
        };
        let packet = MerchantOffers { container: 4, offers: vec![offer, MerchantOffer::default()], level: 2, xp: 15, show_progress: true, can_restock: true };
        let mut bytes = Vec::new();
        packet.write(&mut bytes);
        let read = MerchantOffers::read(&mut &bytes[..]).expect("reads back");
        assert_eq!(read, packet);
        // 10 + floor(10 * 3 * 0.05) - 2
        assert_eq!(read.offers[0].price_a(), 9);
        assert_eq!(read.offers[1].price_a(), 1);
    }
}
