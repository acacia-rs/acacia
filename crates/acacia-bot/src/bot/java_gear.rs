//! What the worn armour gives the Java player's physics.

use acacia_java_client::Inventory;
use acacia_java_physics::Player;
use acacia_java_proto::item::ItemStack;
use acacia_world::neutral::Item;

/// Window 0's chest and feet armour slots.
const CHEST_SLOT: usize = 6;
const FEET_SLOT: usize = 8;
const ELYTRA_DURABILITY: i32 = 432;

/// Sets the player's glider and powder-snow boots from the armour slots.
pub(super) fn apply_worn(player: &mut Player, inventory: &Inventory) {
    player.has_glider = usable_elytra(inventory.slot(CHEST_SLOT));
    player.snow_boots = inventory.slot(FEET_SLOT).is_some_and(|s| s.item == i32::from(Item::LeatherBoots.java_id()));
}

/// An elytra that will not break on its next use (`LivingEntity.canGlideUsing`).
fn usable_elytra(chest: Option<&ItemStack>) -> bool {
    chest.is_some_and(|s| {
        let max = s.components.max_damage.unwrap_or(ELYTRA_DURABILITY);
        i32::from(Item::Elytra.java_id()) == s.item && s.components.damage.unwrap_or(0) < max - 1
    })
}
