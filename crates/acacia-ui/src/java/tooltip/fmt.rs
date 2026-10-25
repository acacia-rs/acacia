//! `ChatFormatting` and `Component` building as tooltip code uses them.

use serde_json::{Map, Value};

use crate::java::text::{Lang, Style, Text, named_color};

pub const DARK_GREEN: u32 = 0x00AA00;
pub const DARK_PURPLE: u32 = 0xAA00AA;
pub const GRAY: u32 = 0xAAAAAA;
pub const DARK_GRAY: u32 = 0x555555;
pub const BLUE: u32 = 0x5555FF;
pub const RED: u32 = 0xFF5555;

/// `Component.translatable(key, args)`.
pub fn tr(key: &str, args: &[Text], lang: Lang) -> Text {
    Text::translate(key, args, lang)
}

/// A number argument (`String.valueOf`).
pub fn num(n: impl ToString) -> Text {
    Text::literal(n.to_string())
}

/// A `ChatFormatting` by name as a style (`withStyle(formatting)`).
pub fn formatting(name: &str) -> Style {
    let on = Some(true);
    match name {
        "bold" => Style { bold: on, ..Style::default() },
        "italic" => Style { italic: on, ..Style::default() },
        "underline" => Style { underlined: on, ..Style::default() },
        "strikethrough" => Style { strikethrough: on, ..Style::default() },
        "obfuscated" => Style { obfuscated: on, ..Style::default() },
        color => named_color(color).map(Style::color).unwrap_or_default(),
    }
}

pub fn gray(t: Text) -> Text {
    t.with_color(GRAY)
}

/// `withStyle(ITALIC, GRAY)`.
pub fn italic_gray(t: Text) -> Text {
    t.with_style(&Style { color: Some(GRAY), italic: Some(true), ..Style::default() })
}

pub fn italic(t: Text) -> Text {
    t.with_style(&formatting("italic"))
}

/// `Component.literal(prefix).append(t)`.
pub fn indent(prefix: &str, t: Text) -> Text {
    Text::literal(prefix).append(t)
}

const STYLE_KEYS: [&str; 8] = ["color", "shadow_color", "bold", "italic", "underlined", "strikethrough", "obfuscated", "font"];

/// Component JSON `v` with `style`'s style fields on its root: over its own when `overwrite`
/// (`withStyle(Style)`), else only where unset (`ComponentUtils.mergeStyles`).
pub fn restyle(v: &Value, style: &Value, overwrite: bool) -> Value {
    let mut root = match v {
        Value::Object(o) => o.clone(),
        Value::Array(list) => Map::from_iter([("text".to_owned(), Value::from("")), ("extra".to_owned(), Value::Array(list.clone()))]),
        other => Map::from_iter([("text".to_owned(), other.clone())]),
    };
    for key in STYLE_KEYS {
        if let Some(s) = style.get(key)
            && (overwrite || !root.contains_key(key))
        {
            root.insert(key.to_owned(), s.clone());
        }
    }
    Value::Object(root)
}

/// `DecimalFormat("#.##")` (half-even on the double's exact value).
pub fn decimal(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "-0".to_owned() } else { s.to_owned() }
}
