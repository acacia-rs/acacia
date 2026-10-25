//! An item's tooltip lines as `ItemStack.getTooltipLines` builds them: the styled name, then
//! `addDetailsToTooltip`'s lines in its order (each component's `addToTooltip` unless the
//! `tooltip_display` component hides it), advanced details and warnings.

use serde_json::Value;

use super::fmt::{BLUE, DARK_GRAY, GRAY, RED, formatting, num, tr};
use super::game::{Game, Player};
use super::stack::ItemStack;
use super::{attributes, contents, details, effects, enchantments, fireworks, names};
use crate::java::text::{Lang, Text};

pub use super::names::{hover_name, rarity_color, styled_name};

/// What `getTooltipLines` reads besides the stack: `Item.TooltipContext` and `TooltipFlag`.
pub struct Context<'a> {
    pub lang: Lang<'a>,
    /// `TooltipFlag.isAdvanced` (F3+H).
    pub advanced: bool,
    /// `TooltipFlag.isCreative`.
    pub creative: bool,
}

/// [`Context`] with the registry facts, the player and the stack's key (its enchantment order).
pub struct Details<'a> {
    pub lang: Lang<'a>,
    pub advanced: bool,
    pub creative: bool,
    pub game: &'a Game,
    pub player: Option<Player>,
    pub key: Option<i32>,
}

impl Details<'_> {
    pub fn tr(&self, key: &str, args: &[Text]) -> Text {
        tr(key, args, self.lang)
    }
}

static NO_FACTS: Game = Game(Value::Null);

/// `TooltipDisplay`: whether a component's lines show.
pub struct Display<'a>(Option<&'a Value>);

impl<'a> Display<'a> {
    pub fn of(stack: &'a ItemStack) -> Display<'a> {
        Display(stack.get("tooltip_display"))
    }

    pub fn hides_tooltip(&self) -> bool {
        self.0.and_then(|d| d.get("hide_tooltip")).and_then(Value::as_bool).unwrap_or(false)
    }

    /// `shows(type)` for a component id without its namespace.
    pub fn shows(&self, component: &str) -> bool {
        let hidden = self.0.and_then(|d| d.get("hidden_components")).and_then(Value::as_array);
        let full = format!("minecraft:{component}");
        !self.hides_tooltip() && !hidden.into_iter().flatten().any(|h| h.as_str() == Some(&full))
    }
}

/// `getTooltipLines(context, player, flag)` without registry facts or a player.
pub fn lines(stack: &ItemStack, cx: &Context) -> Vec<Text> {
    tooltip_lines(stack, &Details { lang: cx.lang, advanced: cx.advanced, creative: cx.creative, game: &NO_FACTS, player: None, key: None })
}

/// `getTooltipLines(context, player, flag)`.
pub fn tooltip_lines(stack: &ItemStack, d: &Details) -> Vec<Text> {
    let display = Display::of(stack);
    let mut out = Vec::new();
    if !d.creative && display.hides_tooltip() {
        if op_warning(stack, d) {
            push_op_warning(&mut out, d);
        }
        return out;
    }
    out.push(names::styled_name(stack, d.lang, d.game));
    add_details(stack, &display, d, &mut out);
    out
}

/// `addDetailsToTooltip`.
pub fn add_details(stack: &ItemStack, display: &Display, d: &Details, out: &mut Vec<Text>) {
    details::hover_text(stack, display, d, out);
    for (component, add) in PROVIDERS {
        if let Some(v) = stack.get(component)
            && display.shows(component)
        {
            add(stack, v, d, out);
        }
    }
    attributes::lines(stack, display, d, out);
    for (component, key, color) in [("intangible_projectile", "item.intangible", GRAY), ("unbreakable", "item.unbreakable", BLUE)] {
        if stack.has(component) && display.shows(component) {
            out.push(d.tr(key, &[]).with_color(color));
        }
    }
    for (component, add) in LATE_PROVIDERS {
        if let Some(v) = stack.get(component)
            && display.shows(component)
        {
            add(stack, v, d, out);
        }
    }
    if matches!(stack.id.as_str(), "minecraft:spawner" | "minecraft:trial_spawner") && display.shows("block_entity_data") {
        details::spawner(stack.get("block_entity_data"), d, out);
    }
    for (component, header) in [("can_break", "item.canBreak"), ("can_place_on", "item.canPlace")] {
        if let Some(v) = stack.get(component)
            && display.shows(component)
        {
            out.push(Text::default());
            out.push(d.tr(header, &[]).with_color(GRAY));
            details::adventure(v, d, out);
        }
    }
    if d.advanced {
        let damage = stack.int("damage", 0);
        if damage > 0 && stack.has("max_damage") && display.shows("damage") {
            let max = stack.int("max_damage", 0);
            out.push(d.tr("item.durability", &[num(max - damage), num(max)]));
        }
        out.push(Text::literal(stack.id.clone()).with_color(DARK_GRAY));
        if stack.size > 0 {
            out.push(d.tr("item.components", &[num(stack.size)]).with_color(DARK_GRAY));
        }
    }
    if d.player.is_some() && !d.game.item_enabled(&stack.id) {
        out.push(d.tr("item.disabled", &[]).with_color(RED));
    }
    if op_warning(stack, d) {
        push_op_warning(out, d);
    }
}

type Provider = fn(&ItemStack, &Value, &Details, &mut Vec<Text>);

/// The `TooltipProvider` components before the attribute lines, in `addDetailsToTooltip`'s order.
const PROVIDERS: [(&str, Provider); 21] = [
    ("tropical_fish/pattern", details::tropical_fish),
    ("instrument", contents::instrument),
    ("map_id", details::map_id),
    ("bees", details::bees),
    ("container_loot", |_, _, d, out| out.push(d.tr("item.container.loot_table.unknown", &[]))),
    ("container", contents::container),
    ("banner_patterns", contents::banner_patterns),
    ("pot_decorations", contents::pot_decorations),
    ("written_book_content", details::written_book),
    ("charged_projectiles", contents::charged_projectiles),
    ("fireworks", fireworks::fireworks),
    ("firework_explosion", fireworks::explosion),
    ("potion_contents", effects::potion_contents),
    ("jukebox_playable", contents::jukebox),
    ("trim", contents::trim),
    ("stored_enchantments", enchantments::stored),
    ("enchantments", enchantments::enchantments),
    ("dyed_color", details::dyed_color),
    ("profile", details::profile),
    ("lore", details::lore),
    ("sulfur_cube_content", contents::sulfur_cube),
];

/// Those after the unit components.
const LATE_PROVIDERS: [(&str, Provider); 6] = [
    ("sign_text_front", |_, v, d, out| details::sign_text(v, "sign.front_text", d, out)),
    ("sign_text_back", |_, v, d, out| details::sign_text(v, "sign.back_text", d, out)),
    ("ominous_bottle_amplifier", effects::ominous_bottle),
    ("suspicious_stew_effects", effects::suspicious_stew),
    ("block_state", details::block_state),
    ("entity_data", details::entity_data),
];

/// `Item.shouldPrintOpWarning`: block items and spawn eggs whose block entity or entity data
/// only an operator may set, for an operator.
fn op_warning(stack: &ItemStack, d: &Details) -> bool {
    if !d.player.is_some_and(|p| p.op) {
        return false;
    }
    let block_item = d.game.entry("item", &stack.id).and_then(|i| i.get("blockItem")).and_then(Value::as_bool).unwrap_or(false);
    let (component, table) = match () {
        _ if stack.id.ends_with("_spawn_egg") => ("entity_data", "entity_type"),
        _ if block_item => ("block_entity_data", "block_entity_type"),
        _ => return false,
    };
    let Some(id) = stack.get(component).and_then(|v| v.get("id")).and_then(Value::as_str) else { return false };
    d.game.entry(table, id).and_then(|e| e.get("op")).and_then(Value::as_bool).unwrap_or(false)
}

fn push_op_warning(out: &mut Vec<Text>, d: &Details) {
    let red = formatting("red");
    out.push(d.tr("item.op_warning.line1", &[]).with_style(&red).with_style(&formatting("bold")));
    out.push(d.tr("item.op_warning.line2", &[]).with_style(&red));
    out.push(d.tr("item.op_warning.line3", &[]).with_style(&red));
}
