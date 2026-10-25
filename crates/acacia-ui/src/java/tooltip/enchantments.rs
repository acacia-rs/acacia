//! `ItemEnchantments.addToTooltip` (the `enchantments` and `stored_enchantments` components):
//! entries in the `tooltip_order` tag's order, then the rest in the map's own order, each as
//! `Enchantment.getFullname`. Also `LevelBasedValue`, which enchantment effects are written in.

use serde_json::{Map, Value, json};

use super::item::Details;
use super::stack::ItemStack;
use crate::java::text::Text;

/// The component's (enchantment, level) entries in its map's iteration order: the oracle's
/// recorded order for a keyed stack (Java's is a hash order), else the JSON's.
pub fn iteration<'a>(stack: &'a ItemStack, component: &str, d: &'a Details) -> Vec<(&'a str, i32)> {
    let Some(map) = stack.get(component).and_then(Value::as_object) else { return Vec::new() };
    let level = |id: &str| map.get(id).and_then(Value::as_i64).unwrap_or(0) as i32;
    match d.key.and_then(|k| d.game.enchantment_order(k, component)) {
        Some(order) => order.into_iter().map(|id| (id, level(id))).collect(),
        None => map.iter().map(|(id, v)| (id.as_str(), v.as_i64().unwrap_or(0) as i32)).collect(),
    }
}

/// `Enchantment.getFullname`: the description in gray (red for a curse) unless it has its own
/// colour, then the level numeral unless both it and the maximum are 1.
pub fn full_name(id: &str, level: i32, d: &Details) -> Text {
    let e = d.game.entry("enchantment", id);
    let description = e.and_then(|e| e.get("description")).cloned().unwrap_or_else(|| json!({"translate": id}));
    let mut root: Map<String, Value> = match description {
        Value::Object(o) => o,
        Value::Array(list) => Map::from_iter([("text".to_owned(), json!("")), ("extra".to_owned(), Value::Array(list))]),
        other => Map::from_iter([("text".to_owned(), other)]),
    };
    let curse = e.and_then(|e| e.get("curse")).and_then(Value::as_bool).unwrap_or(false);
    root.entry("color").or_insert_with(|| json!(if curse { "red" } else { "gray" }));
    let max = e.and_then(|e| e.get("max_level")).and_then(Value::as_i64).unwrap_or(1);
    if level != 1 || max != 1 {
        let extra = root.entry("extra").or_insert_with(|| json!([]));
        if let Value::Array(extra) = extra {
            extra.push(json!(" "));
            extra.push(json!({"translate": format!("enchantment.level.{level}")}));
        }
    }
    Text::from_json(&Value::Object(root), d.lang)
}

fn add(stack: &ItemStack, component: &str, d: &Details, out: &mut Vec<Text>) {
    let entries = iteration(stack, component, d);
    let order: Vec<&str> = d.game.tooltip_order().collect();
    for id in &order {
        if let Some(&(_, level)) = entries.iter().find(|(e, _)| e == id)
            && level > 0
        {
            out.push(full_name(id, level, d));
        }
    }
    for (id, level) in entries {
        if !order.contains(&id) {
            out.push(full_name(id, level, d));
        }
    }
}

pub fn enchantments(stack: &ItemStack, _: &Value, d: &Details, out: &mut Vec<Text>) {
    add(stack, "enchantments", d, out);
}

pub fn stored(stack: &ItemStack, _: &Value, d: &Details, out: &mut Vec<Text>) {
    add(stack, "stored_enchantments", d, out);
}

/// `LevelBasedValue.calculate(level)` of its JSON.
pub fn level_value(v: &Value, level: i32) -> f32 {
    let f = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let sub = |k: &str| v.get(k).map_or(0.0, |x| level_value(x, level));
    let Some(kind) = v.get("type").and_then(Value::as_str) else { return v.as_f64().unwrap_or(0.0) as f32 };
    match kind.strip_prefix("minecraft:").unwrap_or(kind) {
        "constant" => f("value"),
        "linear" => f("base") + f("per_level_above_first") * (level - 1) as f32,
        "levels_squared" => (level * level) as f32 + f("added"),
        "clamped" => sub("value").clamp(f("min"), f("max")),
        "fraction" => {
            let denominator = sub("denominator");
            if denominator == 0.0 { 0.0 } else { sub("numerator") / denominator }
        }
        "exponent" => f64::from(sub("base")).powf(f64::from(sub("power"))) as f32,
        "lookup" => {
            let values = v.get("values").and_then(Value::as_array);
            match values.and_then(|vs| vs.get((level - 1) as usize)).filter(|_| level >= 1) {
                Some(x) => x.as_f64().unwrap_or(0.0) as f32,
                None => sub("fallback"),
            }
        }
        _ => 0.0,
    }
}
