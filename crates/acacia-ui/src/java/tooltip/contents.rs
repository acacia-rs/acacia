//! Providers that name other items or registry entries: container contents, charged projectiles
//! (with each projectile's own details), pot decorations, sulfur cube contents, banner patterns,
//! armor trims, jukebox songs and instruments.

use serde_json::{Value, json};

use super::fmt::{GRAY, gray, indent, italic, italic_gray, num, restyle};
use super::item::{self, Details, Display};
use super::names::{display_name, hover_name};
use super::stack::ItemStack;
use crate::java::text::{Span, Style, Text};

fn named(v: &Value, d: &Details) -> Text {
    let stack = d.game.stack(v);
    stack.map(|s| hover_name(&s, d.lang, d.game)).unwrap_or_else(|| Text::literal(v.get("id").and_then(Value::as_str).unwrap_or("?")))
}

/// `ItemContainerContents.addToTooltip`: "count x name" for the first five stacks, then how many
/// more there are.
pub fn container(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let mut slots: Vec<&Value> = v.as_array().into_iter().flatten().collect();
    slots.sort_by_key(|s| s.get("slot").and_then(Value::as_i64).unwrap_or(0));
    let (mut lines, mut items) = (0, 0);
    for s in slots {
        let Some(template) = s.get("item") else { continue };
        items += 1;
        if lines <= 4 {
            lines += 1;
            let stack = d.game.stack(template);
            let count = stack.as_ref().map_or(1, |s| s.count);
            out.push(d.tr("item.container.item_count", &[num(count), named(template, d)]));
        }
    }
    if items > lines {
        out.push(italic(d.tr("item.container.more_items", &[num(items - lines)])));
    }
}

/// `ChargedProjectiles.addToTooltip`: runs of equal projectiles, each named then followed by its
/// own details (no player, normal flags), indented in gray.
pub fn charged_projectiles(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let list = v.as_array().map_or(&[][..], Vec::as_slice);
    for run in list.chunk_by(|a, b| a == b) {
        let Some(projectile) = d.game.stack(&run[0]) else { continue };
        out.push(match run.len() {
            1 => d.tr("item.minecraft.crossbow.projectile.single", &[display_name(&projectile, d.lang, d.game)]),
            n => d.tr("item.minecraft.crossbow.projectile.multiple", &[num(n), hover_name(&projectile, d.lang, d.game)]),
        });
        let inner = Details { lang: d.lang, advanced: false, creative: false, game: d.game, player: None, key: None };
        let mut lines = Vec::new();
        item::add_details(&projectile, &Display::of(&projectile), &inner, &mut lines);
        out.extend(lines.into_iter().map(|l| indent("  ", l).with_color(GRAY)));
    }
}

/// `Component.plainCopy()`: the contents without style or siblings, for an item name.
fn plain(t: Text) -> Text {
    Text { spans: t.spans.into_iter().map(|s| Span { text: s.text, style: Style::default() }).collect(), forward: false }
}

pub fn pot_decorations(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let sides: Vec<&Value> = ["front", "left", "right", "back"].iter().filter_map(|k| v.get(*k)).collect();
    if sides.is_empty() {
        return;
    }
    out.push(Text::default());
    out.extend(sides.into_iter().map(|s| gray(plain(named(s, d)))));
}

pub fn sulfur_cube(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    out.push(italic_gray(d.tr("entity.minecraft.sulfur_cube.content", &[named(v, d)])));
}

/// `BannerPatternLayers.addToTooltip`: the first six layers.
pub fn banner_patterns(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    for layer in v.as_array().into_iter().flatten().take(6) {
        let pattern = match &layer["pattern"] {
            Value::String(id) => d.game.entry("banner_pattern", id),
            inline => Some(inline),
        };
        let prefix = pattern.and_then(|p| p.get("translation_key")).and_then(Value::as_str).unwrap_or("?");
        let color = layer.get("color").and_then(Value::as_str).unwrap_or("white");
        out.push(gray(d.tr(&format!("{prefix}.{color}"), &[])));
    }
}

/// A registry entry's `description` (component JSON), inline or by id.
fn description(v: &Value, table: &str, d: &Details) -> Value {
    let entry = match v {
        Value::String(id) => d.game.entry(table, id),
        inline => Some(inline),
    };
    entry.and_then(|e| e.get("description")).cloned().unwrap_or_else(|| json!(v.as_str().unwrap_or("?")))
}

/// `ArmorTrim.addToTooltip`: the upgrade title, the pattern in the material's style, the material.
pub fn trim(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let material = description(&v["material"], "trim_material", d);
    let pattern = restyle(&description(&v["pattern"], "trim_pattern", d), &material, true);
    out.push(gray(d.tr("item.minecraft.smithing_template.upgrade", &[])));
    out.push(indent(" ", Text::from_json(&pattern, d.lang)));
    out.push(indent(" ", Text::from_json(&material, d.lang)));
}

/// A description under `mergeStyles(GRAY)`.
fn gray_description(v: &Value, table: &str, d: &Details) -> Text {
    Text::from_json(&restyle(&description(v, table, d), &json!({"color": "gray"}), false), d.lang)
}

pub fn jukebox(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let song = v.get("song").unwrap_or(v);
    out.push(gray_description(song, "jukebox_song", d));
}

pub fn instrument(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    out.push(gray_description(v, "instrument", d));
}
