//! Component -> JSON, the form of status descriptions and login disconnect reasons.

use std::fmt::Write;

use super::{Component, Content};

impl Component {
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        if let Some(text) = self.plain_text() {
            return write_string(text, out);
        }
        out.push('{');
        match &self.content {
            Content::Text(text) => field("text", out, |o| write_string(text, o), true),
            Content::Translatable { key, fallback, args } => {
                field("translate", out, |o| write_string(key, o), true);
                if let Some(fallback) = fallback {
                    field("fallback", out, |o| write_string(fallback, o), false);
                }
                if !args.is_empty() {
                    field("with", out, |o| write_list(args, o), false);
                }
            }
        }
        let style = &self.style;
        if let Some(color) = style.color {
            field("color", out, |o| write_string(&color.serialize(), o), false);
        }
        for (name, value) in [
            ("bold", style.bold),
            ("italic", style.italic),
            ("underlined", style.underlined),
            ("strikethrough", style.strikethrough),
            ("obfuscated", style.obfuscated),
        ] {
            if let Some(on) = value {
                field(name, out, |o| o.push_str(if on { "true" } else { "false" }), false);
            }
        }
        if !self.extra.is_empty() {
            field("extra", out, |o| write_list(&self.extra, o), false);
        }
        out.push('}');
    }
}

fn field(name: &str, out: &mut String, value: impl FnOnce(&mut String), first: bool) {
    if !first {
        out.push(',');
    }
    write_string(name, out);
    out.push(':');
    value(out);
}

fn write_list(items: &[Component], out: &mut String) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        item.write_json(out);
    }
    out.push(']');
}

fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
