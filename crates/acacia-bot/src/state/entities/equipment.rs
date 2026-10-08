use acacia_client::proto::packets::{MobArmorEquipment, MobEquipment};
use acacia_client::proto::types::{ItemV4, WindowID};

use crate::state::ItemStack;

/// What an entity holds and wears, as the server last sent it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Equipment {
    pub main_hand: ItemStack,
    pub off_hand: ItemStack,
    /// Helmet, chestplate, leggings, boots.
    pub armor: [ItemStack; 4],
    /// Horse, wolf or llama armour.
    pub body: ItemStack,
}

impl Equipment {
    pub(super) fn holding(item: ItemV4) -> Option<Box<Self>> {
        let main_hand: ItemStack = item.into();
        (!main_hand.is_empty()).then(|| Box::new(Equipment { main_hand, ..Equipment::default() }))
    }

    pub(super) fn apply_hand(&mut self, p: MobEquipment) {
        let hand = if p.window_id == WindowID::Offhand { &mut self.off_hand } else { &mut self.main_hand };
        *hand = p.item.into();
    }

    pub(super) fn apply_armor(&mut self, p: MobArmorEquipment) {
        self.armor = [p.helmet.into(), p.chestplate.into(), p.leggings.into(), p.boots.into()];
        self.body = p.body.into();
    }
}
