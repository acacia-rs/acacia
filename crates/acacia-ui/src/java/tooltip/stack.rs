//! An item stack as the Java game holds it: the item id, the count and every data component the
//! stack has (its item's defaults with its own patch applied), each in its JSON form
//! (`DataComponentMap.CODEC`). The GUI reads only these.

use serde_json::{Map, Value};

#[derive(Clone, Debug, Default)]
pub struct ItemStack {
    pub id: String,
    pub count: i32,
    /// By full component id (`minecraft:rarity`).
    pub components: Map<String, Value>,
    /// `components.size()`, non-persisted components included.
    pub size: usize,
    /// The draw key the stack was given, which names its recorded enchantment order.
    pub key: Option<i32>,
}

/// Equal stacks: the key only labels where the stack was drawn.
impl PartialEq for ItemStack {
    fn eq(&self, o: &ItemStack) -> bool {
        self.id == o.id && self.count == o.count && self.components == o.components && self.size == o.size
    }
}

impl ItemStack {
    /// `{id, count, components, size}`; `None` for anything else.
    pub fn from_json(v: &Value) -> Option<ItemStack> {
        let components = v.get("components").and_then(Value::as_object).cloned().unwrap_or_default();
        Some(ItemStack {
            id: v.get("id")?.as_str()?.to_owned(),
            count: v.get("count").and_then(Value::as_i64).unwrap_or(1) as i32,
            size: v.get("size").and_then(Value::as_u64).map_or(components.len(), |n| n as usize),
            components,
            key: None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.id == "minecraft:air"
    }

    /// A component by its id without the `minecraft:` namespace.
    pub fn get(&self, component: &str) -> Option<&Value> {
        self.components.get(&format!("minecraft:{component}"))
    }

    pub fn has(&self, component: &str) -> bool {
        self.get(component).is_some()
    }

    pub fn int(&self, component: &str, fallback: i32) -> i32 {
        self.get(component).and_then(Value::as_i64).map_or(fallback, |v| v as i32)
    }

    /// `getMaxStackSize`.
    pub fn max_stack_size(&self) -> i32 {
        self.int("max_stack_size", 1)
    }

    /// `copyWithCount`.
    pub fn with_count(&self, count: i32) -> ItemStack {
        ItemStack { count, ..self.clone() }
    }
}
