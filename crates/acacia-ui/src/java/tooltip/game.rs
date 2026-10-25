//! The registry facts item tooltips read beyond the stack itself, as the game holds them: data-pack
//! entries (enchantments, instruments, jukebox songs, trims, banner patterns, paintings) in their
//! codec's JSON, built-in ones by the few fields read (`{description, ...}`), each by full id under
//! its registry's name; block tags' members, the `tooltip_order` enchantment tag, each keyed stack's
//! enchantment iteration order and the player's base attack values. A client fills it from what the
//! server sent; the `tooltips` harness from the oracle's `game`.

use serde_json::{Map, Value};

use super::stack::ItemStack;

#[derive(Clone, Debug, Default)]
pub struct Game(pub Value);

/// The player `getTooltipLines` is given (`Screen.getTooltipFromItem` passes the local player).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    /// `getAttributeBaseValue(ATTACK_DAMAGE)`, `(ATTACK_SPEED)`.
    pub attack_damage: f64,
    pub attack_speed: f64,
    /// Has `COMMANDS_GAMEMASTER`.
    pub op: bool,
}

impl Game {
    pub fn new(v: Value) -> Game {
        Game(v)
    }

    /// The entry `id` of registry `table` (`enchantment`, `mob_effect`, ...).
    pub fn entry(&self, table: &str, id: &str) -> Option<&Value> {
        self.0.get(table)?.get(id)
    }

    /// An entry's string field: `description` by default for built-in entries.
    pub fn text(&self, table: &str, id: &str, field: &str) -> Option<&str> {
        self.entry(table, id)?.get(field)?.as_str()
    }

    /// The `tooltip_order` enchantment tag's entries.
    pub fn tooltip_order(&self) -> impl Iterator<Item = &str> {
        self.0.get("tooltipOrder").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str)
    }

    /// The members of block tag `#ns:path`.
    pub fn block_tag(&self, tag: &str) -> Vec<&str> {
        let members = self.0.get("blockTags").and_then(|t| t.get(tag)).and_then(Value::as_array);
        members.into_iter().flatten().filter_map(Value::as_str).collect()
    }

    /// The iteration order of keyed stack `key`'s `enchantments` or `stored_enchantments` map.
    pub fn enchantment_order(&self, key: i32, component: &str) -> Option<Vec<&str>> {
        let order = self.0.get("enchantmentOrder")?.get(key.to_string())?.get(component)?.as_array()?;
        Some(order.iter().filter_map(Value::as_str).collect())
    }

    pub fn player(&self, op: bool) -> Player {
        let p = self.0.get("player");
        let f = |k: &str, d: f64| p.and_then(|p| p.get(k)).and_then(Value::as_f64).unwrap_or(d);
        Player { attack_damage: f("attack_damage", 1.0), attack_speed: f("attack_speed", 4.0), op }
    }

    /// `ItemStackTemplate.create()`: the item's default components with `template`'s patch
    /// (`!id` removes) applied; `None` for an unknown item.
    pub fn stack(&self, template: &Value) -> Option<ItemStack> {
        let id = match template {
            Value::String(id) => id.as_str(),
            t => t.get("id")?.as_str()?,
        };
        let mut components: Map<String, Value> = self.entry("item", id)?.get("components")?.as_object()?.clone();
        for (k, v) in template.get("components").and_then(Value::as_object).into_iter().flatten() {
            match k.strip_prefix('!') {
                Some(removed) => {
                    components.remove(removed);
                }
                None => {
                    components.insert(k.clone(), v.clone());
                }
            }
        }
        let count = template.get("count").and_then(Value::as_i64).unwrap_or(1) as i32;
        let stack = ItemStack { id: id.to_owned(), count, size: components.len(), components, key: None };
        // ItemStack.validateStrict: an overstacked template creates the empty stack (air).
        if count > stack.max_stack_size() {
            let name = self.entry("item", "minecraft:air").and_then(|a| a.get("components")?.get("minecraft:item_name")).cloned();
            let name = name.unwrap_or_else(|| serde_json::json!({"translate": "block.minecraft.air"}));
            let components = Map::from_iter([("minecraft:item_name".to_owned(), name)]);
            return Some(ItemStack { id: "minecraft:air".to_owned(), count: 0, size: 1, components, key: None });
        }
        Some(stack)
    }

    /// `Item.isEnabled` under the default feature flags.
    pub fn item_enabled(&self, id: &str) -> bool {
        self.entry("item", id).and_then(|i| i.get("enabled")).and_then(Value::as_bool).unwrap_or(true)
    }
}
