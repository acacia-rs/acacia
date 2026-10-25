//! The `creative` harness: `tools/parity/harnesses/creative/cases.py`'s cases with what
//! `CreativeOracle` found of the game in `game` (the tabs, the menu's items, the hovered stack's
//! tabs and tags) and the hovered and carried stacks resolved in `items`.

use serde_json::{Map, Value, json};

use acacia_ui::java::container::creative::{Creative, Tab, TabType};
use acacia_ui::java::container::entity::Figure;
use acacia_ui::java::container::{CARRIED, Env, frame};
use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::{Lang, Text};
use acacia_ui::java::tooltip::stack::ItemStack;

use crate::Assets;
use crate::kit;

fn tab(t: &Value, lang: Lang) -> Tab {
    Tab {
        kind: match t["type"].as_str() {
            Some("HOTBAR") => TabType::Hotbar,
            Some("SEARCH") => TabType::Search,
            Some("INVENTORY") => TabType::Inventory,
            _ => TabType::Category,
        },
        top: t["row"] == "TOP",
        column: kit::int(t, "column", 0),
        right: kit::bool(t, "right"),
        show_title: kit::bool(t, "showTitle"),
        can_scroll: kit::bool(t, "canScroll"),
        background: t["background"].as_str().unwrap_or_default().to_owned(),
        title: Text::from_json(&t["title"], lang),
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}

/// The player figure's render state as `CreativeStubs.Figure` fills it from the case.
fn figure(c: &Value) -> Figure {
    let f = &c["figure"];
    Figure { entity: "minecraft:player".into(), bb_height: kit::float(f, "bbHeight", 0.0), scale: Some(kit::float(f, "scale", 1.0)), fall_flying: kit::bool(f, "fallFlying") }
}

pub fn replay(c: &Value, assets: &Assets, _state: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = kit::lang(assets);
    let env = Env { font: assets.font.as_ref(), lang: &lang, advanced: kit::bool(c, "advanced") };
    let g = &c["game"];
    let tabs: Vec<Tab> = g["tabs"].as_array().into_iter().flatten().map(|t| tab(t, &lang)).collect();
    let ids: Vec<&str> = g["tabs"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    // init: a tab that is not shown falls back to the default one.
    let selected = ids.iter().position(|&id| c["tab"] == id).or_else(|| ids.iter().position(|&id| id == "minecraft:building_blocks"));
    let Some(selected) = selected else { return Vec::new() };
    let (width, height) = (kit::int(c, "width", 0), kit::int(c, "height", 0));
    let mut screen = Creative::new(width, height, tabs, selected, figure(c));
    for e in c["inventory"].as_array().into_iter().flatten() {
        if let Some(stack) = ItemStack::from_json(&e[1]).filter(|s| !s.is_empty()) {
            screen.set_inventory(e[0].as_u64().unwrap_or(0) as usize, stack);
        }
    }
    let empty: Vec<u64> = g["empty"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
    let present = |n: usize| (!empty.contains(&(n as u64))).then(|| ItemStack::from_json(&json!({"id": "minecraft:stone", "count": 1}))).flatten();
    screen.set_items(kit::int(g, "menu", 0) as usize, present, kit::float(c, "scroll", 0.0));
    if let Some(s) = c.get("search").filter(|_| screen.tab().kind == TabType::Search) {
        let font = env.font;
        screen.search.set_value(font, s["text"].as_str().unwrap_or_default());
        screen.search.set_cursor(font, kit::int(s, "cursor", 0).max(0) as usize);
        screen.search.set_highlight(font, kit::int(s, "highlight", 0).max(0) as usize);
        screen.search.blink_on = kit::int(s, "blink", 0) % 2 == 0;
    }
    for (key, _) in c["items"].as_object().into_iter().flatten() {
        let key: i32 = key.parse().unwrap_or(-1);
        if let Some(slot) = usize::try_from(key).ok().and_then(|k| screen.base.slots.get_mut(k)) {
            slot.item = kit::item(c, key);
        }
    }
    screen.base.carried = kit::item(c, CARRIED);
    screen.effects = kit::effects(c);
    screen.member = g["member"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|i| i as usize).collect();
    screen.visible_tags = strings(&g["visibleTags"]);
    screen.item_tags = strings(&g["tags"]);
    let mut gui = Gui::new(width, height);
    frame(&mut screen, &mut gui, &env, kit::mouse(c), kit::float(c, "partial", 0.0));
    gui.draws
}
