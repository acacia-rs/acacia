use acacia_api::{ItemCount, Recipe};
use acacia_bot::state::{Ingredient, ItemRegistry, ItemStack, Recipe as BotRecipe};

/// `stone` → `minecraft:stone`; namespaced names pass unchanged.
pub(crate) fn item_name(name: &str) -> String {
    if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") }
}

/// An item's name, or `#<network id>` for one the item table lacks.
pub(crate) fn stack_name(items: &ItemRegistry, stack: &ItemStack) -> String {
    items.name(stack.network_id).map_or_else(|| format!("#{}", stack.network_id), str::to_owned)
}

/// A recipe with its grid cells summed per ingredient, in first-seen order.
pub(crate) fn recipe(items: &ItemRegistry, r: &BotRecipe) -> Recipe {
    let mut inputs: Vec<ItemCount> = Vec::new();
    for cell in r.inputs.iter().filter(|i| !i.is_empty()) {
        let item = match cell {
            Ingredient::Item { name, .. } => name.clone(),
            Ingredient::Tag { tag, .. } if tag.starts_with('#') => tag.clone(),
            Ingredient::Tag { tag, .. } => format!("#{tag}"),
            Ingredient::Molang { expression, .. } => expression.clone(),
            Ingredient::Empty => continue,
        };
        match inputs.iter_mut().find(|i| i.item == item) {
            Some(known) => known.count += u32::from(cell.count()),
            None => inputs.push(ItemCount { item, count: u32::from(cell.count()) }),
        }
    }
    let outputs = r.outputs.iter().map(|s| ItemCount { item: stack_name(items, s), count: u32::from(s.count) }).collect();
    Recipe { id: r.id.clone(), station: r.block.clone(), fits_2x2: r.fits_2x2(), inputs, outputs }
}
