//! The smaller tooltip providers: `Item.appendHoverText` overrides (smithing templates, disc
//! fragments, paintings), spawners, adventure-mode predicates, entity data, block state, bees,
//! tropical fish, sign text, profiles, map ids, dyed colour, lore and written books.

use serde_json::{Value, json};

use super::fmt::{BLUE, DARK_GRAY, RED, gray, indent, italic_gray, num};
use super::item::{Details, Display};
use super::stack::ItemStack;
use crate::java::text::{Style, Text};

/// `getItem().appendHoverText(...)`.
pub fn hover_text(stack: &ItemStack, display: &Display, d: &Details, out: &mut Vec<Text>) {
    let path = stack.id.strip_prefix("minecraft:").unwrap_or(&stack.id);
    if path.ends_with("_smithing_template") {
        let kind = if path == "netherite_upgrade_smithing_template" { "netherite_upgrade" } else { "armor_trim" };
        let key = |k: &str| format!("item.minecraft.smithing_template{k}");
        out.push(gray(d.tr(&key(""), &[])));
        out.push(Text::default());
        out.push(gray(d.tr(&key(".applies_to"), &[])));
        out.push(indent(" ", d.tr(&key(&format!(".{kind}.applies_to")), &[]).with_color(BLUE)));
        out.push(gray(d.tr(&key(".ingredients"), &[])));
        out.push(indent(" ", d.tr(&key(&format!(".{kind}.ingredients")), &[]).with_color(BLUE)));
    } else if path == "disc_fragment_5" {
        out.push(gray(d.tr("item.minecraft.disc_fragment_5.desc", &[])));
    } else if path == "painting" && display.shows("painting/variant") {
        match stack.get("painting/variant").and_then(|v| painting(v, d)) {
            Some(variant) => {
                for field in ["title", "author"] {
                    if let Some(t) = variant.get(field) {
                        out.push(Text::from_json(t, d.lang));
                    }
                }
                let size = |k: &str| num(variant.get(k).and_then(Value::as_i64).unwrap_or(1));
                out.push(d.tr("painting.dimensions", &[size("width"), size("height")]));
            }
            None if d.creative => out.push(gray(d.tr("painting.random", &[]))),
            None => {}
        }
    }
}

fn painting<'a>(v: &'a Value, d: &'a Details) -> Option<&'a Value> {
    match v {
        Value::String(id) => d.game.entry("painting_variant", id),
        inline => Some(inline),
    }
}

fn entity_name(id: &str, d: &Details) -> Text {
    let path = id.strip_prefix("minecraft:").unwrap_or(id);
    let key = d.game.text("entity_type", id, "description").map_or_else(|| format!("entity.minecraft.{path}"), str::to_owned);
    d.tr(&key, &[])
}

/// `Spawner.appendHoverText(blockEntityData, "SpawnData")`.
pub fn spawner(data: Option<&Value>, d: &Details, out: &mut Vec<Text>) {
    let entity = data.and_then(|v| v.get("SpawnData")?.get("entity")?.get("id")?.as_str());
    match entity {
        Some(id) => out.push(gray(entity_name(id, d))),
        None => {
            out.push(Text::default());
            out.push(gray(d.tr("block.minecraft.spawner.desc1", &[])));
            out.push(indent(" ", d.tr("block.minecraft.spawner.desc2", &[]).with_color(BLUE)));
        }
    }
}

/// `AdventureModePredicate.addToTooltip`: every predicate's blocks (tags expanded), distinct,
/// as dark gray names; "Unknown" when a predicate names no blocks.
pub fn adventure(v: &Value, d: &Details, out: &mut Vec<Text>) {
    let predicates: Vec<&Value> = match v {
        Value::Array(list) => list.iter().collect(),
        single => vec![single],
    };
    if predicates.iter().any(|p| p.get("blocks").is_none()) {
        out.push(gray(d.tr("item.canUse.unknown", &[])));
        return;
    }
    let mut blocks: Vec<&str> = Vec::new();
    for p in predicates {
        let ids: Vec<&str> = match &p["blocks"] {
            Value::String(tag) if tag.starts_with('#') => d.game.block_tag(tag),
            Value::String(id) => vec![id.as_str()],
            Value::Array(list) => list.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        for id in ids {
            if !blocks.contains(&id) {
                blocks.push(id);
            }
        }
    }
    for id in blocks {
        let path = id.strip_prefix("minecraft:").unwrap_or(id);
        let key = d.game.text("block", id, "description").map_or_else(|| format!("block.minecraft.{path}"), str::to_owned);
        out.push(d.tr(&key, &[]).with_color(DARK_GRAY));
    }
}

/// `TypedEntityData.addToTooltip`: a warning for a mob a peaceful world will not spawn.
pub fn entity_data(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let peaceful = d.game.0.get("peaceful").and_then(Value::as_bool).unwrap_or(false);
    let Some(id) = v.get("id").and_then(Value::as_str) else { return };
    let allowed = d.game.entry("entity_type", id).and_then(|e| e.get("peaceful")).and_then(Value::as_bool).unwrap_or(true);
    if peaceful && !allowed {
        out.push(d.tr("item.spawn_egg.peaceful", &[]).with_color(RED));
    }
}

/// `BlockItemStateProperties.addToTooltip`: a beehive's honey level.
pub fn block_state(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let level = v.get("honey_level").and_then(Value::as_str).and_then(|s| s.parse::<i32>().ok()).filter(|l| (0..=5).contains(l));
    if let Some(level) = level {
        out.push(gray(d.tr("container.beehive.honey", &[num(level), num(5)])));
    }
}

pub fn bees(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let n = v.as_array().map_or(0, Vec::len);
    out.push(gray(d.tr("container.beehive.bees", &[num(n), num(3)])));
}

/// `TropicalFish.COMMON_VARIANTS`: (pattern, base colour, pattern colour).
const COMMON_FISH: [(&str, &str, &str); 22] = [
    ("stripey", "orange", "gray"), ("flopper", "gray", "gray"), ("flopper", "gray", "blue"), ("clayfish", "white", "gray"),
    ("sunstreak", "blue", "gray"), ("kob", "orange", "white"), ("spotty", "pink", "light_blue"), ("blockfish", "purple", "yellow"),
    ("clayfish", "white", "red"), ("spotty", "white", "yellow"), ("glitter", "white", "gray"), ("clayfish", "white", "orange"),
    ("dasher", "cyan", "pink"), ("brinely", "lime", "light_blue"), ("betty", "red", "white"), ("snooper", "gray", "red"),
    ("blockfish", "red", "white"), ("flopper", "white", "yellow"), ("kob", "red", "white"), ("sunstreak", "gray", "white"),
    ("dasher", "cyan", "yellow"), ("flopper", "yellow", "yellow"),
];

/// `TropicalFish.Pattern.addToTooltip`: a common variant's name, else the pattern and colours.
pub fn tropical_fish(stack: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let pattern = v.as_str().unwrap_or("kob");
    let color = |k: &str| stack.get(k).and_then(Value::as_str).unwrap_or("white");
    let (base, over) = (color("tropical_fish/base_color"), color("tropical_fish/pattern_color"));
    if let Some(i) = COMMON_FISH.iter().position(|&f| f == (pattern, base, over)) {
        out.push(italic_gray(d.tr(&format!("entity.minecraft.tropical_fish.predefined.{i}"), &[])));
        return;
    }
    out.push(italic_gray(d.tr(&format!("entity.minecraft.tropical_fish.type.{pattern}"), &[])));
    let mut colors = d.tr(&format!("color.minecraft.{base}"), &[]);
    if base != over {
        colors = colors.append(", ").append(d.tr(&format!("color.minecraft.{over}"), &[]));
    }
    out.push(italic_gray(colors));
}

/// `SignText.createTooltip(title)`: nothing for `SignText.EMPTY`.
pub fn sign_text(v: &Value, title: &str, d: &Details, out: &mut Vec<Text>) {
    let messages = v.get("messages").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let blank = |m: &Value| m.as_str() == Some("") || m == &json!({"text": ""});
    let empty = messages.iter().all(blank)
        && v.get("filtered_messages").is_none()
        && v.get("color").and_then(Value::as_str).unwrap_or("black") == "black"
        && !v.get("has_glowing_text").and_then(Value::as_bool).unwrap_or(false);
    if empty {
        return;
    }
    out.push(gray(d.tr(title, &[])));
    out.extend(messages.iter().map(|m| indent("  ", Text::from_json(m, d.lang))));
}

/// `ResolvableProfile.Dynamic.addToTooltip`: a profile named only by name or only by id.
pub fn profile(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let dynamic = match v {
        Value::String(_) => true,
        p => {
            let props = p.get("properties").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
            !props && p.get("id").is_some() != p.get("name").is_some()
        }
    };
    if dynamic {
        out.push(gray(d.tr("component.profile.dynamic", &[])));
    }
}

/// `MapId.addToTooltip`, with the level's map data (`maps`: [id, scale, locked]).
pub fn map_id(stack: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let id = v.as_i64().unwrap_or(0);
    let maps = d.game.0.get("maps").and_then(Value::as_array);
    let Some(data) = maps.into_iter().flatten().find(|m| m[0].as_i64() == Some(id)) else {
        out.push(gray(d.tr("filled_map.unknown", &[])));
        return;
    };
    let post = stack.get("map_post_processing").and_then(Value::as_str);
    if !stack.has("custom_name") && post.is_none() {
        out.push(gray(d.tr("filled_map.id", &[num(id)])));
    }
    if data[2].as_bool().unwrap_or(false) || post == Some("lock") {
        out.push(gray(d.tr("filled_map.locked", &[])));
    }
    if d.advanced {
        let scale = (data[1].as_i64().unwrap_or(0) + i64::from(post == Some("scale"))).min(4);
        out.push(gray(d.tr("filled_map.scale", &[num(1 << scale)])));
        out.push(gray(d.tr("filled_map.level", &[num(scale), num(4)])));
    }
}

pub fn dyed_color(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    if d.advanced {
        let rgb = v.as_i64().or_else(|| v.get("rgb").and_then(Value::as_i64)).unwrap_or(0);
        out.push(gray(d.tr("item.color", &[Text::literal(format!("#{:06X}", rgb & 0xFFFFFF))])));
    } else {
        out.push(italic_gray(d.tr("item.dyed", &[])));
    }
}

/// `ItemLore.styledLines`: each line under dark purple italic.
pub fn lore(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let style = Style { color: Some(super::fmt::DARK_PURPLE), italic: Some(true), ..Style::default() };
    out.extend(v.as_array().into_iter().flatten().map(|l| Text::from_json(l, d.lang).with_style(&style)));
}

pub fn written_book(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    if let Some(author) = v.get("author").and_then(Value::as_str).filter(|a| !a.trim().is_empty()) {
        out.push(gray(d.tr("book.byAuthor", &[Text::literal(author)])));
    }
    let generation = v.get("generation").and_then(Value::as_i64).unwrap_or(0);
    out.push(gray(d.tr(&format!("book.generation.{generation}"), &[])));
}
