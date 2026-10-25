//! An item stack's names: `getHoverName`, `getStyledHoverName`, `getDisplayName` and the
//! `Item.getName` overrides (potions, tipped arrows, player heads, lodestone compasses, shields).

use serde_json::Value;

use super::fmt::tr;
use super::game::Game;
use super::stack::ItemStack;
use crate::java::text::{Lang, Style, Text};

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

/// `getCustomName`: the `custom_name` component, else a written book's non-blank title.
pub fn custom_name(stack: &ItemStack, lang: Lang) -> Option<Text> {
    if let Some(name) = stack.get("custom_name") {
        return Some(Text::from_json(name, lang));
    }
    let title = stack.get("written_book_content")?.get("title")?;
    let raw = title.get("raw").and_then(Value::as_str).or(title.as_str())?;
    (!raw.trim().is_empty()).then(|| Text::literal(raw))
}

/// `Item.getName(stack)`.
pub fn item_name(stack: &ItemStack, lang: Lang, game: &Game) -> Text {
    let path = stack.id.strip_prefix("minecraft:").unwrap_or(&stack.id);
    match path {
        "potion" | "splash_potion" | "lingering_potion" | "tipped_arrow" => {
            if let Some(contents) = stack.get("potion_contents") {
                return tr(&format!("item.minecraft.{path}.effect.{}", potion_suffix(contents, game)), &[], lang);
            }
        }
        "player_head" => {
            if let Some(name) = stack.get("profile").and_then(profile_name) {
                return tr("block.minecraft.player_head.named", &[Text::literal(name)], lang);
            }
        }
        "compass" if stack.has("lodestone_tracker") => return tr("item.minecraft.lodestone_compass", &[], lang),
        "shield" => {
            if let Some(color) = stack.get("base_color").and_then(Value::as_str) {
                return tr(&format!("item.minecraft.shield.{color}"), &[], lang);
            }
        }
        _ => {}
    }
    stack.get("item_name").map(|n| Text::from_json(n, lang)).unwrap_or_default()
}

/// `PotionContents.getName`'s suffix: the custom name, else the potion's name, else `empty`.
fn potion_suffix<'a>(contents: &'a Value, game: &'a Game) -> &'a str {
    let custom = contents.get("custom_name").and_then(Value::as_str);
    let potion = || contents.get("potion").and_then(Value::as_str).and_then(|p| game.text("potion", p, "name"));
    custom.or_else(potion).unwrap_or("empty")
}

/// `ResolvableProfile.name()`.
fn profile_name(profile: &Value) -> Option<&str> {
    match profile {
        Value::String(s) => Some(s),
        p => p.get("name")?.as_str(),
    }
}

/// `getHoverName`: the custom name, else the item name.
pub fn hover_name(stack: &ItemStack, lang: Lang, game: &Game) -> Text {
    custom_name(stack, lang).unwrap_or_else(|| item_name(stack, lang, game))
}

/// `getStyledHoverName`: in the rarity's colour, italic when custom named.
pub fn styled_name(stack: &ItemStack, lang: Lang, game: &Game) -> Text {
    let mut style = Style::color(rarity_color(stack));
    if stack.has("custom_name") {
        style.italic = Some(true);
    }
    hover_name(stack, lang, game).with_style(&style)
}

/// `getDisplayName`: the hover name (italic when custom named) in square brackets, in the
/// rarity's colour.
pub fn display_name(stack: &ItemStack, lang: Lang, game: &Game) -> Text {
    let mut name = hover_name(stack, lang, game);
    if stack.has("custom_name") {
        name = super::fmt::italic(name);
    }
    let wrapped = tr("chat.square_brackets", &[name], lang);
    if stack.is_empty() { wrapped } else { wrapped.with_color(rarity_color(stack)) }
}
