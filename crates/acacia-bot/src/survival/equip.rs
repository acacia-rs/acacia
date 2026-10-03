use std::time::Duration;

use crate::human::{CLICK, HOTBAR_SWITCH};
use crate::items::SlotRef;
use crate::state::{Inventory, ItemStack};
use crate::{ActionError, Bot};

/// How long to wait for the server to move a used armour piece into its slot.
const WEAR_TIMEOUT: Duration = Duration::from_secs(3);

/// How [`Bot::equip_by`] puts on armour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EquipMethod {
    /// Through the inventory screen: `Place` into the armour slot, or `Swap` with what is worn.
    #[default]
    Inventory,
    /// Holding the piece and using it (`UseItem` ClickAir), as a right-click in the world does; the
    /// server swaps it with what is worn. A single piece's swap is predicted, not awaited.
    Use,
}

/// Where [`Bot::equip`] puts an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// Into the hotbar and selected.
    Hand,
    Head,
    Chest,
    Legs,
    Feet,
    OffHand,
}

impl Destination {
    fn armor_slot(self) -> Option<u8> {
        match self {
            Destination::Head => Some(0),
            Destination::Chest => Some(1),
            Destination::Legs => Some(2),
            Destination::Feet => Some(3),
            Destination::Hand | Destination::OffHand => None,
        }
    }
}

/// The armour slot (0 helmet .. 3 boots) an item identifier can be worn in.
pub(crate) fn armor_slot(name: &str) -> Option<u8> {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    if n.ends_with("_helmet") || n == "carved_pumpkin" || n.ends_with("_head") || n.ends_with("_skull") {
        Some(0)
    } else if n.ends_with("_chestplate") || n == "elytra" {
        Some(1)
    } else if n.ends_with("_leggings") {
        Some(2)
    } else if n.ends_with("_boots") {
        Some(3)
    } else {
        None
    }
}

/// The hotbar slot an item from the inventory goes to: the first empty one, else the selected one.
pub(crate) fn hotbar_target(inv: &Inventory) -> u8 {
    (0..Inventory::HOTBAR_SLOTS).find(|&i| inv.main[usize::from(i)].is_empty()).unwrap_or(inv.selected_hotbar_slot)
}

impl Bot {
    /// The first main-inventory slot holding `name` (`minecraft:bread`), hotbar first.
    pub fn find_item(&self, name: &str) -> Option<SlotRef> {
        let id = self.state.items.id(name)?;
        let main = &self.state.inventory.main;
        (0..main.len()).find(|&i| !main[i].is_empty() && main[i].network_id == id).map(|i| SlotRef::Main(i as u8))
    }

    /// Puts the item in `from` on (armour, offhand) through the inventory screen, or brings it to
    /// the hotbar and selects it (`Hand`). A worn item is swapped back into `from`.
    pub async fn equip(&mut self, from: SlotRef, to: Destination) -> Result<(), ActionError> {
        self.equip_by(from, to, EquipMethod::Inventory).await
    }

    /// [`Bot::equip`] choosing how armour goes on. With [`EquipMethod::Use`] the piece is first
    /// brought to the hand, and a worn piece ends up in the hand's slot.
    pub async fn equip_by(&mut self, from: SlotRef, to: Destination, method: EquipMethod) -> Result<(), ActionError> {
        let stack = from.stack(&self.state).filter(|s| !s.is_empty()).cloned();
        let stack = stack.ok_or_else(|| ActionError::NotPossible(format!("{from:?} is empty")))?;
        let Some(slot) = to.armor_slot() else {
            return match (to, method) {
                (_, EquipMethod::Use) => Err(ActionError::NotPossible(format!("only armour can be put on by use, not {to:?}"))),
                (Destination::Hand, _) => self.equip_hand(from, &stack).await,
                _ => self.wear(from, &stack, SlotRef::Offhand, whole(&stack)).await,
            };
        };
        let name = self.state.items.name(stack.network_id).unwrap_or_default();
        if armor_slot(name) != Some(slot) {
            return Err(ActionError::NotPossible(format!("{name} cannot be worn as {to:?}")));
        }
        match method {
            EquipMethod::Inventory => self.wear(from, &stack, SlotRef::Armor(slot), 1).await,
            EquipMethod::Use => self.wear_by_use(from, &stack, slot).await,
        }
    }

    async fn wear_by_use(&mut self, from: SlotRef, stack: &ItemStack, slot: u8) -> Result<(), ActionError> {
        if from != SlotRef::Main(self.state.inventory.selected_hotbar_slot) {
            self.equip_hand(from, stack).await?;
            let pause = self.human.between(HOTBAR_SWITCH);
            self.pause(pause).await?;
        }
        let before = self.state.inventory.armor[usize::from(slot)].clone();
        self.use_item();
        // BDS swaps hand and armour slot without telling this client.
        if stack.count == 1 {
            let inv = &mut self.state.inventory;
            let hand = usize::from(inv.selected_hotbar_slot);
            inv.main[hand] = std::mem::replace(&mut inv.armor[usize::from(slot)], inv.main[hand].clone());
            return Ok(());
        }
        let worn = self.wait_until(WEAR_TIMEOUT, |bot, _| {
            let armor = &bot.state.inventory.armor[usize::from(slot)];
            (*armor != before && armor.network_id == stack.network_id).then_some(())
        });
        match worn.await {
            Err(ActionError::Timeout) => Err(ActionError::Rejected("the server did not put the armour on".into())),
            other => other,
        }
    }

    async fn wear(&mut self, from: SlotRef, stack: &ItemStack, to: SlotRef, count: u8) -> Result<(), ActionError> {
        if from == to {
            return Ok(());
        }
        if to.stack(&self.state).is_some_and(|s| !s.is_empty()) {
            if u16::from(count) < stack.count {
                return Err(ActionError::NotPossible(format!("{to:?} is occupied; move one item from the stack first")));
            }
            return self.swap_items(from, to).await;
        }
        self.move_item(from, to, count).await
    }

    async fn equip_hand(&mut self, from: SlotRef, stack: &ItemStack) -> Result<(), ActionError> {
        if from == SlotRef::Main(self.state.inventory.selected_hotbar_slot) {
            return Ok(());
        }
        let target = match from {
            SlotRef::Main(i) if i < Inventory::HOTBAR_SLOTS => i,
            _ => {
                let target = hotbar_target(&self.state.inventory);
                self.wear(from, stack, SlotRef::Main(target), whole(stack)).await?;
                target
            }
        };
        let pause = self.human.between(CLICK);
        self.pause(pause).await?;
        // Also resent when the selection is unchanged: the held item changed, and Boar checks uses
        // against the item from the last MobEquipment.
        self.select_hotbar(target)
    }
}

fn whole(stack: &ItemStack) -> u8 {
    u8::try_from(stack.count).unwrap_or(u8::MAX)
}
