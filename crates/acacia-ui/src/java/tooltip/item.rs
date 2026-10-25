//! An item's tooltip lines as `ItemStack.getTooltipLines` builds them.

use serde_json::Value;

use super::stack::ItemStack;
use crate::java::text::{Lang, Style, Text};

const DARK_GRAY: u32 = 0x555555;

/// What `getTooltipLines` reads besides the stack: `Item.TooltipContext` and `TooltipFlag`.
pub struct Context<'a> {
    pub lang: Lang<'a>,
    /// `TooltipFlag.isAdvanced` (F3+H).
    pub advanced: bool,
    /// `TooltipFlag.isCreative`.
    pub creative: bool,
}

/// `Rarity.color()` of `getRarity`: the `rarity` component, raised one step when enchanted.
pub fn rarity_color(stack: &ItemStack) -> u32 {
    let base = stack.get("rarity").and_then(Value::as_str).unwrap_or("common");
    let enchanted = stack.get("enchantments").and_then(Value::as_object).is_some_and(|e| !e.is_empty());
    let rarity = match (base, enchanted) {
        ("common" | "uncommon", true) => "rare",
        ("rare", true) => "epic",
        (r, _) => r,
    };
    match rarity {
        "uncommon" => 0xFFFF55,
        "rare" => 0x55FFFF,
        "epic" => 0xFF55FF,
        _ => 0xFFFFFF,
    }
}

/// `getHoverName`: the custom name, else the item name.
pub fn hover_name(stack: &ItemStack, lang: Lang) -> Text {
    let name = stack.get("custom_name").or_else(|| stack.get("item_name"));
    name.map(|n| Text::from_json(n, lang)).unwrap_or_else(|| Text::literal(stack.id.clone()))
}

/// `getStyledHoverName`: in the rarity's colour, italic when custom named.
pub fn styled_name(stack: &ItemStack, lang: Lang) -> Text {
    let mut style = Style::color(rarity_color(stack));
    if stack.has("custom_name") {
        style.italic = Some(true);
    }
    hover_name(stack, lang).with_style(&style)
}

fn hides_tooltip(stack: &ItemStack) -> bool {
    stack.get("tooltip_display").and_then(|d| d.get("hide_tooltip")).and_then(Value::as_bool).unwrap_or(false)
}

/// `getTooltipLines(context, player, flag)`.
pub fn lines(stack: &ItemStack, cx: &Context) -> Vec<Text> {
    if !cx.creative && hides_tooltip(stack) {
        return Vec::new();
    }
    let mut out = vec![styled_name(stack, cx.lang)];
    // TODO: addDetailsToTooltip's component lines (lore, enchantments, attributes, ...).
    if cx.advanced {
        let damage = stack.int("damage", 0);
        let max = stack.int("max_damage", 0);
        if damage > 0 && stack.has("max_damage") {
            out.push(Text::translate("item.durability", &[Text::literal((max - damage).to_string()), Text::literal(max.to_string())], cx.lang));
        }
        out.push(Text::literal(stack.id.clone()).with_color(DARK_GRAY));
        if stack.size > 0 {
            out.push(Text::translate("item.components", &[Text::literal(stack.size.to_string())], cx.lang).with_color(DARK_GRAY));
        }
    }
    out
}
