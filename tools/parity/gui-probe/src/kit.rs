//! What the screen harnesses' replays share: the game's translations, a case's resolved stacks
//! (`ScreenKit`'s probe input: `items` by key) and its container state.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use acacia_ui::java::container::effects::Effect;
use acacia_ui::java::container::{Base, CARRIED, QuickCraft};
use acacia_ui::java::text::Text;
use acacia_ui::java::tooltip::game::Game;
use acacia_ui::java::tooltip::stack::ItemStack;

use crate::Assets;

static LANG: OnceLock<HashMap<String, String>> = OnceLock::new();

/// `lang/en_us.json`'s translation of `key`, with `lang/deprecated.json` applied as the game loads
/// it (`DeprecatedTranslationsInfo.applyToMap`: removed keys dropped, renamed keys moved).
pub fn lang(assets: &Assets) -> impl Fn(&str) -> Option<String> + '_ {
    let map = LANG.get_or_init(|| {
        let read = |name: &str| std::fs::read_to_string(assets.root.join(name)).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
        let mut map: HashMap<String, String> = read("lang/en_us.json").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        if let Some(deprecated) = read("lang/deprecated.json") {
            for key in deprecated["removed"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                map.remove(key);
            }
            for (from, to) in deprecated["renamed"].as_object().into_iter().flatten() {
                let to = to.as_str().unwrap_or_default().to_owned();
                match map.remove(from) {
                    Some(value) => map.insert(to, value),
                    None => map.remove(&to),
                };
            }
        }
        map
    });
    move |key: &str| map.get(key).cloned()
}

pub fn int(c: &Value, key: &str, fallback: i32) -> i32 {
    c[key].as_i64().map_or(fallback, |v| v as i32)
}

pub fn float(c: &Value, key: &str, fallback: f32) -> f32 {
    c[key].as_f64().map_or(fallback, |v| v as f32)
}

pub fn bool(c: &Value, key: &str) -> bool {
    c[key].as_bool().unwrap_or(false)
}

pub fn mouse(c: &Value) -> [i32; 2] {
    [c["mouse"][0].as_i64().unwrap_or(0) as i32, c["mouse"][1].as_i64().unwrap_or(0) as i32]
}

/// The case's registry facts (`TooltipFacts`, written for every screen case).
pub fn game(c: &Value) -> Game {
    Game::new(c["game"].clone())
}

/// The resolved stack under `key`.
pub fn item(c: &Value, key: i32) -> Option<ItemStack> {
    c["items"].get(key.to_string()).and_then(ItemStack::from_json).map(|s| ItemStack { key: Some(key), ..s })
}

/// The case's `title` component, or `fallback` translated.
pub fn title(c: &Value, fallback: &str, lang: &dyn Fn(&str) -> Option<String>) -> Text {
    match &c["title"] {
        Value::Null => Text::translate(fallback, &[], lang),
        t => Text::from_json(t, lang),
    }
}

/// The case's `effects` with their colours from `game.effectColors` (`ScreenKit.applyEffects`).
pub fn effects(c: &Value) -> Vec<Effect> {
    let effects = c["effects"].as_array().into_iter().flatten();
    effects
        .map(|e| {
            let id = e[0].as_str().unwrap_or_default().to_owned();
            let color = c["game"]["effectColors"][&id].as_i64().unwrap_or(0) as i32;
            Effect { duration: e[1].as_i64().unwrap_or(0) as i32, amplifier: e[2].as_i64().unwrap_or(0) as i32, ambient: e[3].as_bool().unwrap_or(false), color, id }
        })
        .collect()
}

/// `ContainerOracle.fill`: slot contents, the carried stack and a quick-craft drag.
pub fn fill(c: &Value, base: &mut Base) {
    for e in c["slots"].as_array().into_iter().flatten() {
        let index = e[0].as_i64().unwrap_or(-1);
        if let Some(slot) = base.slots.get_mut(index as usize) {
            slot.item = item(c, index as i32);
        }
    }
    base.carried = item(c, CARRIED);
    if let Some(q) = c.get("quick") {
        let slots = q["slots"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|i| i as usize).filter(|&i| i < base.slots.len());
        base.quick = Some(QuickCraft { kind: int(q, "type", 0), slots: slots.collect(), remainder: 0 });
        base.recalculate_quick_craft();
    }
}
