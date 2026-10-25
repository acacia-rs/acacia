//! The viewer's stacks as the Java screens read them (`ItemStack`): the Java item
//! `JavaItems::java_name` names, the count, and the components a Bedrock stack carries (a given
//! name, enchantments, lore, dye, banner layers). An item's other default components are not known
//! here: every stack gets its item's name key and [`MAX_STACK`], and no rarity (common).

use acacia_render::item::JavaItems;
use acacia_ui::java::tooltip::game::Game;
use acacia_ui::java::tooltip::stack::ItemStack;
use serde_json::{Map, Value, json};

/// The game's translations by key.
pub type JavaLang = std::collections::HashMap<String, String>;
use crate::control::Stack;
use crate::enchant_names::JAVA as ENCHANTMENTS;

/// `max_stack_size` for every item: Bedrock's item table does not say, and most items stack to 64.
pub const MAX_STACK: i32 = 64;

/// `DyeColor`s by id, white first (as `Banner`'s dyes count).
pub const DYES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red",
    "black",
];

/// The Java item a stack is: `minecraft:apple`.
pub fn java_id(stack: &Stack, items: Option<&JavaItems>) -> String {
    items.map_or_else(|| stack.name.clone(), |items| items.java_name(&stack.name, stack.aux).to_owned())
}

/// `stack` as the screens read it, drawn under `key`.
pub fn item_stack(stack: &Stack, items: Option<&JavaItems>, lang: &JavaLang, key: i32) -> ItemStack {
    let id = java_id(stack, items);
    let path = id.strip_prefix("minecraft:").unwrap_or(&id);
    let mut c = Map::new();
    // Blocks' items are named by their block's key.
    let name = ["item", "block"].map(|kind| format!("{kind}.minecraft.{path}")).into_iter().find(|k| lang.contains_key(k));
    c.insert("minecraft:item_name".into(), json!({"translate": name.unwrap_or_else(|| format!("item.minecraft.{path}"))}));
    c.insert("minecraft:max_stack_size".into(), json!(MAX_STACK));
    if let Some(title) = &stack.title {
        c.insert("minecraft:custom_name".into(), json!(title));
    }
    let enchantments: Map<String, Value> = stack
        .enchantments
        .iter()
        .filter_map(|&(id, level)| Some((format!("minecraft:{}", ENCHANTMENTS.get(usize::try_from(id).ok()?)?.0), json!(level))))
        .collect();
    if !enchantments.is_empty() {
        let component = if path == "enchanted_book" { "minecraft:stored_enchantments" } else { "minecraft:enchantments" };
        c.insert(component.into(), Value::Object(enchantments));
    }
    if !stack.lore.is_empty() {
        c.insert("minecraft:lore".into(), json!(stack.lore));
    }
    if let Some([r, g, b]) = stack.dye {
        c.insert("minecraft:dyed_color".into(), json!(i32::from(r) << 16 | i32::from(g) << 8 | i32::from(b)));
    }
    if let Some(banner) = &stack.banner {
        let layer = |&(pattern, dye): &(&str, u8)| json!({"pattern": format!("minecraft:{pattern}"), "color": DYES[usize::from(dye) % 16]});
        c.insert("minecraft:banner_patterns".into(), Value::Array(banner.layers.iter().map(layer).collect()));
    }
    if let Some(color) = path.strip_suffix("_dye").filter(|c| DYES.contains(c)) {
        c.insert("minecraft:dye".into(), json!(color));
    }
    ItemStack { id, count: i32::from(stack.count), size: c.len(), components: c, key: Some(key) }
}

/// The registry facts tooltips read that the viewer has: the enchantments' names, maximum levels
/// and curses. Everything else is absent (no tags, no tooltip order, base attack values).
pub fn facts() -> Game {
    let entry = |&(id, max): &(&str, i32)| {
        let curse = id.ends_with("_curse");
        (format!("minecraft:{id}"), json!({"description": {"translate": format!("enchantment.minecraft.{id}")}, "max_level": max, "curse": curse}))
    };
    Game::new(json!({ "enchantment": Value::Object(ENCHANTMENTS.iter().map(entry).collect()) }))
}

/// A stack of one plain item (the beacon's payment icons, tab icons).
pub fn plain(name: &str) -> Stack {
    Stack { name: name.to_owned(), aux: 0, count: 1, block: 0, enchanted: false, dye: None, banner: None, map: None, title: None, enchantments: Vec::new(), lore: Vec::new() }
}
