//! `ItemStack.addAttributeTooltips`: per `EquipmentSlotGroup` in declaration order, the
//! `attribute_modifiers` component's modifiers for that group then its enchantments' attribute
//! effects (`EnchantmentHelper.forEachModifier`), under an empty line and the group's header; each
//! modifier as its `ItemAttributeModifiers.Display` says. Ported from Horizon's `item_tooltip.rs` (MIT).

use serde_json::Value;

use super::enchantments::{self, level_value};
use super::fmt::{DARK_GREEN, GRAY, decimal, formatting};
use super::item::{Details, Display};
use super::stack::ItemStack;
use crate::java::text::Text;

/// `EquipmentSlotGroup.values()`.
pub const SLOT_GROUPS: [&str; 11] = ["any", "mainhand", "offhand", "hand", "feet", "legs", "chest", "head", "armor", "body", "saddle"];

/// A modifier: attribute id, modifier id, amount, operation (`add_value`, ...).
pub struct Modifier<'a> {
    pub attribute: &'a str,
    pub id: &'a str,
    pub amount: f64,
    pub operation: &'a str,
}

impl Modifier<'_> {
    /// `AttributeModifier.Operation.id()`.
    pub fn operation_id(&self) -> u8 {
        match self.operation {
            "add_multiplied_base" => 1,
            "add_multiplied_total" => 2,
            _ => 0,
        }
    }

    pub fn multiplied(&self) -> bool {
        self.operation_id() != 0
    }
}

/// `Component.translatable(attribute.getDescriptionId())`.
pub fn attribute_name(attribute: &str, d: &Details) -> Text {
    let path = attribute.strip_prefix("minecraft:").unwrap_or(attribute);
    let key = d.game.text("attribute", attribute, "description").map_or_else(|| format!("attribute.name.{path}"), str::to_owned);
    d.tr(&key, &[])
}

/// `attribute.modifier.{plus,take}.<op>`: none for a zero amount.
pub fn signed_line(m: &Modifier, shown: f64, d: &Details) -> Option<(Text, bool)> {
    let (kind, value, increase) = match m.amount {
        a if a > 0.0 => ("plus", shown, true),
        a if a < 0.0 => ("take", -shown, false),
        _ => return None,
    };
    let key = format!("attribute.modifier.{kind}.{}", m.operation_id());
    Some((d.tr(&key, &[Text::literal(decimal(value)), attribute_name(m.attribute, d)]), increase))
}

/// `Display.Default.apply`.
fn default_line(m: &Modifier, d: &Details) -> Option<Text> {
    let base = d.player.and_then(|p| match m.id {
        "minecraft:base_attack_damage" => Some(p.attack_damage),
        "minecraft:base_attack_speed" => Some(p.attack_speed),
        _ => None,
    });
    let amount = m.amount + base.unwrap_or(0.0);
    let shown = match () {
        _ if m.multiplied() => amount * 100.0,
        _ if m.attribute == "minecraft:knockback_resistance" => amount * 10.0,
        _ => amount,
    };
    if base.is_some() {
        let key = format!("attribute.modifier.equals.{}", m.operation_id());
        let line = d.tr(&key, &[Text::literal(decimal(shown)), attribute_name(m.attribute, d)]);
        return Some(Text::literal(" ").append(line).with_color(DARK_GREEN));
    }
    let m = Modifier { amount, ..*m };
    let (line, increase) = signed_line(&m, shown, d)?;
    let field = if increase { "positive" } else { "negative" };
    let color = d.game.text("attribute", m.attribute, field).unwrap_or(if increase { "blue" } else { "red" });
    Some(line.with_style(&formatting(color)))
}

fn modifier(v: &Value) -> Option<Modifier<'_>> {
    Some(Modifier {
        attribute: v.get("type").or_else(|| v.get("attribute"))?.as_str()?,
        id: v.get("id").and_then(Value::as_str).unwrap_or(""),
        amount: v.get("amount")?.as_f64()?,
        operation: v.get("operation")?.as_str()?,
    })
}

pub fn lines(stack: &ItemStack, display: &Display, d: &Details, out: &mut Vec<Text>) {
    if !display.shows("attribute_modifiers") {
        return;
    }
    let own = stack.get("attribute_modifiers").and_then(Value::as_array);
    for group in SLOT_GROUPS {
        let mut group_lines: Vec<Option<Text>> = Vec::new();
        for v in own.into_iter().flatten() {
            if v.get("slot").and_then(Value::as_str).unwrap_or("any") != group {
                continue;
            }
            let Some(m) = modifier(v) else { continue };
            match v.get("display").and_then(|x| x.get("type")).and_then(Value::as_str).unwrap_or("default") {
                "hidden" => {}
                "override" => group_lines.push(Some(v["display"].get("value").map(|t| Text::from_json(t, d.lang)).unwrap_or_default())),
                _ => group_lines.push(default_line(&m, d)),
            }
        }
        for (enchantment, level) in enchantments::iteration(stack, "enchantments", d) {
            let Some(e) = d.game.entry("enchantment", enchantment) else { continue };
            let in_group = e.get("slots").and_then(Value::as_array).into_iter().flatten().any(|s| s.as_str() == Some(group));
            if !in_group {
                continue;
            }
            let effects = e.get("effects").and_then(|x| x.get("minecraft:attributes")).and_then(Value::as_array);
            for effect in effects.into_iter().flatten() {
                let (Some(attribute), Some(amount), Some(operation)) =
                    (effect.get("attribute").and_then(Value::as_str), effect.get("amount"), effect.get("operation").and_then(Value::as_str))
                else {
                    continue;
                };
                let m = Modifier { attribute, id: "", amount: f64::from(level_value(amount, level)), operation };
                group_lines.push(default_line(&m, d));
            }
        }
        if !group_lines.is_empty() {
            out.push(Text::default());
            out.push(d.tr(&format!("item.modifiers.{group}"), &[]).with_color(GRAY));
            out.extend(group_lines.into_iter().flatten());
        }
    }
}
