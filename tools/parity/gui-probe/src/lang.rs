//! The game's `en_us` translations, loaded once from the assets, and component JSON read through
//! them.

use std::collections::HashMap;
use std::sync::OnceLock;

use acacia_ui::java::text::Text;
use serde_json::Value;

use crate::Assets;

pub fn table(assets: &Assets) -> &'static HashMap<String, String> {
    static LANG: OnceLock<HashMap<String, String>> = OnceLock::new();
    LANG.get_or_init(|| {
        let path = assets.root.join("lang/en_us.json");
        std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
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
