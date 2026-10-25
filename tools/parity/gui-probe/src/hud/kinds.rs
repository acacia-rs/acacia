//! The `hud` harness's text and bar kinds: effects, title, selected, boss, jump, decor.

use serde_json::Value;

use acacia_ui::java::gui::Gui;
use acacia_ui::java::hud::bars::{self, BossBar};
use acacia_ui::java::hud::decor::{self, ItemDecorations};
use acacia_ui::java::hud::effects::{self, EffectIcon};
use acacia_ui::java::hud::titles::{HeldName, Titles, ToolHighlight, background_color};
use acacia_ui::java::text::named_color;

use crate::Assets;
use crate::lang::{comp, translate};

pub fn int(c: &Value, k: &str, d: i64) -> i32 {
    c[k].as_i64().unwrap_or(d) as i32
}

pub fn float(c: &Value, k: &str, d: f64) -> f32 {
    c[k].as_f64().unwrap_or(d) as f32
}

pub fn screen(c: &Value) -> Gui {
    Gui::new(int(c, "width", 0), int(c, "height", 0))
}

/// `Options.getBackgroundColor(default)` from the case's text background options.
pub fn background(c: &Value, default: f32) -> u32 {
    background_color(c["chatOnly"].as_bool().unwrap_or(true), float(c, "textOpacity", 0.5), default)
}

pub fn effects(c: &Value, gui: &mut Gui) {
    let icons: Vec<EffectIcon> = c["active"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            EffectIcon::vanilla(e["id"].as_str()?, int(e, "duration", 0), e["ambient"].as_bool().unwrap_or(false), e["icon"].as_bool().unwrap_or(true))
        })
        .collect();
    effects::effects(gui, c["demo"].as_bool().unwrap_or(false), &icons);
}

/// `Hud.setTitle` / `setSubtitle` / `setTimes` / `clearTitles` / `resetTitleTimes` /
/// `setOverlayMessage` and ticks, then the action bar and the title.
pub fn title(c: &Value, assets: &Assets, gui: &mut Gui) {
    let mut t = Titles::default();
    for op in c["ops"].as_array().into_iter().flatten() {
        let n = |i: usize| op[i].as_i64().unwrap_or(0) as i32;
        match op[0].as_str().unwrap_or_default() {
            "title" => t.set_title(comp(assets, &op[1])),
            "subtitle" => t.set_subtitle(comp(assets, &op[1])),
            "times" => t.set_times(n(1), n(2), n(3)),
            "clear" => t.clear(),
            "reset" => t.reset_times(),
            "overlay" => t.set_overlay(comp(assets, &op[1]), op[2].as_bool().unwrap_or(false)),
            "tick" => (0..n(1)).for_each(|_| t.tick()),
            other => panic!("unknown title op {other}"),
        }
    }
    let (partial, bg) = (float(c, "partial", 0.0), background(c, 0.0));
    t.overlay_message(gui, assets.font.as_ref(), partial, bg);
    t.title(gui, assets.font.as_ref(), partial, bg);
}

/// Ticks `toolHighlightTimer` through the selected stacks, then draws the name.
pub fn selected(c: &Value, assets: &Assets, gui: &mut Gui) {
    let mut highlight = ToolHighlight::default();
    let time = c["displayTime"].as_f64().unwrap_or(1.0);
    for step in c["steps"].as_array().into_iter().flatten() {
        let held = (!step.is_null()).then(|| {
            let custom = &step["custom"];
            HeldName {
                item: step["item"].as_str().unwrap_or_default().to_owned(),
                hover_name: if custom.is_null() { translate(assets, step["key"].as_str().unwrap_or_default()) } else { comp(assets, custom) },
                rarity_color: named_color(step["rarity"].as_str().unwrap_or("white")).unwrap_or(0xFF_FFFF),
                custom_name: !custom.is_null(),
            }
        });
        highlight.tick(held, time);
    }
    let mode = c["gameMode"].as_str().unwrap_or("survival");
    highlight.draw(gui, assets.font.as_ref(), mode == "survival" || mode == "adventure", background(c, 0.0));
}

pub fn boss(c: &Value, assets: &Assets, gui: &mut Gui) {
    let bars: Vec<BossBar> = c["bars"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| BossBar {
            name: comp(assets, &b["name"]),
            progress: float(b, "progress", 0.0),
            color: int(b, "color", 0) as usize,
            overlay: int(b, "overlay", 0) as usize,
        })
        .collect();
    bars::boss_overlay(gui, assets.font.as_ref(), &bars);
}

pub fn jump(c: &Value, gui: &mut Gui) {
    bars::jump_bar(gui, int(c, "cooldown", 0), float(c, "jumpScale", 0.0));
}

/// `itemDecorations` for a stack with a count, damage and a cooldown `elapsed` ticks into
/// `duration`; a count of 0 is an empty stack.
pub fn decor(c: &Value, assets: &Assets, gui: &mut Gui) {
    let count = int(c, "count", 1);
    if count <= 0 {
        return;
    }
    let cd = &c["cooldown"];
    let cooldown = if cd.is_null() { 0.0 } else { decor::cooldown_percent(int(cd, "duration", 0), int(cd, "elapsed", 0), float(c, "partial", 0.0)) };
    let max_damage = if c["unbreakable"].as_bool().unwrap_or(false) { 0 } else { int(c, "maxDamage", 0) };
    let d = ItemDecorations {
        count,
        count_text: c["countText"].as_str().map(str::to_owned),
        bar: decor::damage_bar(int(c, "damage", 0), max_damage),
        cooldown,
    };
    decor::item_decorations(gui, assets.font.as_ref(), int(c, "x", 0), int(c, "y", 0), &d);
}
