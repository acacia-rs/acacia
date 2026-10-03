//! Convenience lookups that combine several trackers.

use std::collections::BTreeMap;

use super::{GameState, ItemStack, Recipe};

impl GameState {
    /// The item's identifier (`minecraft:diamond`), once the item registry has arrived.
    pub fn item_name(&self, stack: &ItemStack) -> Option<&str> {
        self.items.name(stack.network_id)
    }

    pub fn held_item(&self) -> &ItemStack {
        self.inventory.held()
    }

    /// First stack of the given identifier as `(window, slot, stack)`: the player's own windows
    /// first, then the open container.
    pub fn find_item(&self, name: &str) -> Option<(i32, usize, &ItemStack)> {
        let id = self.items.id(name)?;
        self.inventory.find(|s| s.network_id == id).or_else(|| {
            let open = self.containers.open.as_ref()?;
            open.iter().find(|(_, s)| s.network_id == id).map(|(slot, s)| (open.window_id, slot, s))
        })
    }

    /// Recipes that make item `name` (`minecraft:stick`). The first lookup decodes `CraftingData`.
    pub fn recipes_for(&self, name: &str) -> Vec<&Recipe> {
        let Some(id) = self.items.id(name) else { return Vec::new() };
        self.recipes.book().producing(id).collect()
    }

    /// Total count per identifier across the player's own windows, sorted by identifier. Items
    /// missing from the registry are listed as `#<network id>`.
    pub fn inventory_summary(&self) -> Vec<(String, u32)> {
        let mut totals: BTreeMap<String, u32> = BTreeMap::new();
        for (_, _, stack) in self.inventory.iter() {
            let name = self.item_name(stack).map_or_else(|| format!("#{}", stack.network_id), str::to_owned);
            *totals.entry(name).or_default() += u32::from(stack.count);
        }
        totals.into_iter().collect()
    }
}

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
