//! The screen and widget harnesses (`tools/parity/harnesses/screens_*`): case readers shared by
//! their replays.

pub mod layouts;
pub mod menus;
pub mod widgets;

use std::collections::HashMap;
use std::sync::OnceLock;

use acacia_ui::java::text::Text;
use acacia_ui::java::widget::Base;
use serde_json::Value;

use crate::Assets;

static LANG: OnceLock<HashMap<String, String>> = OnceLock::new();

/// `en_us.json`, which the game's `Language` loads from its jar.
pub fn lang(assets: &Assets) -> &'static HashMap<String, String> {
    LANG.get_or_init(|| {
        let path = assets.root.join("lang/en_us.json");
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    })
}

/// A case's component: a string is literal text, anything else component JSON.
pub fn text(v: &Value, lang: &HashMap<String, String>) -> Text {
    match v {
        Value::Null => Text::default(),
        Value::String(s) => Text::literal(s.clone()),
        _ => Text::from_json(v, &|k| lang.get(k).cloned()),
    }
}

pub fn int(c: &Value, k: &str, d: i32) -> i32 {
    c[k].as_i64().map_or(d, |v| v as i32)
}

pub fn float(c: &Value, k: &str, d: f32) -> f32 {
    c[k].as_f64().map_or(d, |v| v as f32)
}

pub fn flag(c: &Value, k: &str, d: bool) -> bool {
    c[k].as_bool().unwrap_or(d)
}

pub fn ints(v: &Value) -> Vec<i32> {
    v.as_array().into_iter().flatten().filter_map(Value::as_i64).map(|v| v as i32).collect()
}

/// The `AbstractWidget` state the oracle writes on every widget.
pub fn common(b: &mut Base, c: &Value) {
    b.active = flag(c, "active", true);
    b.visible = flag(c, "visible", true);
    b.alpha = float(c, "alpha", 1.0);
    b.focused = flag(c, "focused", false);
}
