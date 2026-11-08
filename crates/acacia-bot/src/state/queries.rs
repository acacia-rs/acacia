//! Convenience lookups that combine several trackers.

use std::collections::BTreeMap;

use acacia_world::neutral::Item;

use super::{GameState, ItemStack, Recipe};

impl GameState {
    /// The item's identifier (`minecraft:diamond`), once the item registry has arrived.
    pub fn item_name(&self, stack: &ItemStack) -> Option<&str> {
        self.items.name(stack.network_id)
    }

    pub fn held_item(&self) -> &ItemStack {
        self.inventory.held()
    }

    /// The stack's neutral item; `None` when empty, unregistered or only on Bedrock.
    pub fn item(&self, stack: &ItemStack) -> Option<Item> {
        if stack.is_empty() {
            return None;
        }
        self.items.item(stack.network_id, stack.metadata)
    }

    /// First stack of `item` as `(window, slot, stack)`: the player's own windows first, then the open
    /// container.
    pub fn find_item(&self, item: Item) -> Option<(i32, usize, &ItemStack)> {
        self.inventory.find(|s| self.item(s) == Some(item)).or_else(|| {
            let open = self.containers.open.as_ref()?;
            open.iter().find(|(_, s)| self.item(s) == Some(item)).map(|(slot, s)| (open.window_id, slot, s))
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
