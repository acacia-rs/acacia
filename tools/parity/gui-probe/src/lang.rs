//! The game's `en_us` translations, loaded once from the assets, and component JSON read through
//! them.

use std::collections::HashMap;
use std::sync::OnceLock;

use acacia_ui::java::text::Text;
use serde_json::Value;

use crate::Assets;

fn read(assets: &Assets, file: &str) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(assets.root.join("lang").join(file)).ok()?).ok()
}

/// `Language.loadDefault`: `en_us.json` with `deprecated.json` applied
/// (`DeprecatedTranslationsInfo.applyToMap`).
pub fn table(assets: &Assets) -> &'static HashMap<String, String> {
    static LANG: OnceLock<HashMap<String, String>> = OnceLock::new();
    LANG.get_or_init(|| {
        let mut map: HashMap<String, String> = read(assets, "en_us.json").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let deprecated = read(assets, "deprecated.json").unwrap_or_default();
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
        map
    })
}

/// `ComponentSerialization.CODEC` decoding of `v`.
pub fn comp(assets: &Assets, v: &Value) -> Text {
    let lang = table(assets);
    Text::from_json(v, &|k| lang.get(k).cloned())
}

/// `Component.translatable(key)`.
pub fn translate(assets: &Assets, key: &str) -> Text {
    let lang = table(assets);
    Text::translate(key, &[], &|k| lang.get(k).cloned())
}
