//! Styled text, a flattened `Component`: spans with a `Style`, read from component JSON
//! (`ComponentSerialization`), with translations formatted as `TranslatableContents` does and
//! legacy `§` codes resolved as `StringDecomposer` does. Ported from Horizon's `text.rs` (MIT).

use serde_json::Value;

/// `ChatFormatting` colours by legacy code.
pub const COLORS: [(char, &str, u32); 16] = [
    ('0', "black", 0x000000),
    ('1', "dark_blue", 0x0000AA),
    ('2', "dark_green", 0x00AA00),
    ('3', "dark_aqua", 0x00AAAA),
    ('4', "dark_red", 0xAA0000),
    ('5', "dark_purple", 0xAA00AA),
    ('6', "gold", 0xFFAA00),
    ('7', "gray", 0xAAAAAA),
    ('8', "dark_gray", 0x555555),
    ('9', "blue", 0x5555FF),
    ('a', "green", 0x55FF55),
    ('b', "aqua", 0x55FFFF),
    ('c', "red", 0xFF5555),
    ('d', "light_purple", 0xFF55FF),
    ('e', "yellow", 0xFFFF55),
    ('f', "white", 0xFFFFFF),
];

pub fn named_color(name: &str) -> Option<u32> {
    COLORS.iter().find(|c| c.1 == name).map(|c| c.2)
}

/// `TextColor.parseColor`: a name or `#RRGGBB`.
pub fn parse_color(s: &str) -> Option<u32> {
    named_color(s).or_else(|| s.strip_prefix('#').filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok()))
}

/// A style; `None` inherits from the enclosing one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Option<u32>,
    /// `shadow_color`, ARGB.
    pub shadow_color: Option<u32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    /// A full font id (`minecraft:alt`); the default font when unset.
    pub font: Option<String>,
}

impl Style {
    pub fn color(c: u32) -> Style {
        Style { color: Some(c), ..Style::default() }
    }

    /// Fills unset fields from `base` (`Style.applyTo`).
    pub fn inherit(&mut self, base: &Style) {
        self.color = self.color.or(base.color);
        self.shadow_color = self.shadow_color.or(base.shadow_color);
        self.bold = self.bold.or(base.bold);
        self.italic = self.italic.or(base.italic);
        self.underlined = self.underlined.or(base.underlined);
        self.strikethrough = self.strikethrough.or(base.strikethrough);
        self.obfuscated = self.obfuscated.or(base.obfuscated);
        if self.font.is_none() {
            self.font.clone_from(&base.font);
        }
    }

    pub fn is_bold(&self) -> bool {
        self.bold == Some(true)
    }

    pub fn is_italic(&self) -> bool {
        self.italic == Some(true)
    }

    pub fn is_underlined(&self) -> bool {
        self.underlined == Some(true)
    }

    pub fn is_strikethrough(&self) -> bool {
        self.strikethrough == Some(true)
    }

    pub fn is_obfuscated(&self) -> bool {
        self.obfuscated == Some(true)
    }

    /// `Style.applyLegacyFormat` for code `code` (lower case); `own` is what `§r` returns to.
    pub fn legacy(&self, code: char, own: &Style) -> Style {
        if let Some(c) = COLORS.iter().find(|c| c.0 == code) {
            let off = Some(false);
            return Style { color: Some(c.2), bold: off, italic: off, underlined: off, strikethrough: off, obfuscated: off, ..self.clone() };
        }
        let mut s = self.clone();
        match code {
            'k' => s.obfuscated = Some(true),
            'l' => s.bold = Some(true),
            'm' => s.strikethrough = Some(true),
            'n' => s.underlined = Some(true),
            'o' => s.italic = Some(true),
            'r' => s = own.clone(),
            _ => {}
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    pub spans: Vec<Span>,
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        Text::literal(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text::literal(s)
    }
}

/// Looks a translation key up; `None` shows the key.
pub type Lang<'a> = &'a dyn Fn(&str) -> Option<String>;

impl Text {
    pub fn literal(s: impl Into<String>) -> Text {
        let text = s.into();
        if text.is_empty() {
            return Text::default();
        }
        Text { spans: vec![Span { text, style: Style::default() }] }
    }

    pub fn styled(s: impl Into<String>, style: &Style) -> Text {
        Text::literal(s).with_style(style)
    }

    /// `Component.translatable(key, args)`.
    pub fn translate(key: &str, args: &[Text], lang: Lang) -> Text {
        format_translation(&lang(key).unwrap_or_else(|| key.to_owned()), args)
    }

    pub fn append(mut self, other: impl Into<Text>) -> Text {
        self.spans.extend(other.into().spans);
        self
    }

    /// `style` under every span (`withStyle` on a parent).
    pub fn with_style(mut self, style: &Style) -> Text {
        for span in &mut self.spans {
            span.style.inherit(style);
        }
        self
    }

    pub fn with_color(self, color: u32) -> Text {
        self.with_style(&Style::color(color))
    }

    /// `Component.getString`.
    pub fn plain(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.text.is_empty())
    }

    /// A plain string's `§` codes as styles (`Component.literal` of a legacy string, drawn).
    pub fn legacy(s: &str) -> Text {
        Text::literal(s).resolve_legacy()
    }

    /// `StringDecomposer.iterateFormatted` per span: `§` codes in the contents restyle what follows
    /// (a colour clears decorations, `§r` returns to the span's own style) and are not drawn.
    pub fn resolve_legacy(&self) -> Text {
        if !self.spans.iter().any(|s| s.text.contains('§')) {
            return self.clone();
        }
        let mut out = Text::default();
        for span in &self.spans {
            let mut style = span.style.clone();
            let mut current = String::new();
            let mut chars = span.text.chars();
            while let Some(c) = chars.next() {
                if c != '§' {
                    current.push(c);
                    continue;
                }
                let Some(code) = chars.next() else { break };
                // Java skips one UTF-16 unit: a supplementary character's low surrogate is left, drawn as U+FFFD.
                if code.len_utf16() == 2 {
                    current.push('\u{FFFD}');
                    continue;
                }
                let next = style.legacy(code.to_ascii_lowercase(), &span.style);
                if next != style {
                    if !current.is_empty() {
                        out.spans.push(Span { text: std::mem::take(&mut current), style: style.clone() });
                    }
                    style = next;
                }
            }
            if !current.is_empty() {
                out.spans.push(Span { text: current, style });
            }
        }
        out
    }

    /// `ChatFormatting.stripFormatting`.
    pub fn strip_legacy(&self) -> Text {
        let code = |c: char| matches!(c.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r');
        let spans = self.spans.iter().filter_map(|span| {
            let mut text = String::with_capacity(span.text.len());
            let mut chars = span.text.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '§' && chars.peek().is_some_and(|&n| code(n)) {
                    chars.next();
                } else {
                    text.push(c);
                }
            }
            (!text.is_empty()).then(|| Span { text, style: span.style.clone() })
        });
        Text { spans: spans.collect() }
    }

    /// Characters with their styles.
    pub fn chars(&self) -> impl Iterator<Item = (char, &Style)> {
        self.spans.iter().flat_map(|s| s.text.chars().map(move |c| (c, &s.style)))
    }

    /// Characters `[from, to)`, styles kept.
    pub fn slice(&self, from: usize, to: usize) -> Text {
        let mut out = Text::default();
        let mut index = 0;
        for span in &self.spans {
            let part: String = span.text.chars().filter(|_| {
                index += 1;
                (from..to).contains(&(index - 1))
            }).collect();
            if !part.is_empty() {
                out.spans.push(Span { text: part, style: span.style.clone() });
            }
        }
        out
    }

    /// `ComponentSerialization.CODEC` decoding (text, translate with fallback, extra, lists).
    pub fn from_json(v: &Value, lang: Lang) -> Text {
        match v {
            Value::String(s) => Text::literal(s.clone()),
            Value::Number(n) => Text::literal(n.to_string()),
            Value::Bool(b) => Text::literal(b.to_string()),
            // A list is its first element with the rest as siblings in its style.
            Value::Array(list) => {
                let Some((first, rest)) = list.split_first() else { return Text::default() };
                let style = first.as_object().map(object_style).unwrap_or_default();
                rest.iter().fold(Text::from_json(first, lang), |t, s| t.append(Text::from_json(s, lang).with_style(&style)))
            }
            Value::Object(o) => {
                let style = object_style(o);
                let content = if let Some(t) = o.get("text") {
                    Text::literal(t.as_str().map_or_else(|| t.to_string(), str::to_owned))
                } else if let Some(key) = o.get("translate").and_then(Value::as_str) {
                    let args: Vec<Text> = o.get("with").and_then(Value::as_array).map(|a| a.iter().map(|x| Text::from_json(x, lang)).collect()).unwrap_or_default();
                    match (lang(key), o.get("fallback").and_then(Value::as_str)) {
                        (None, Some(fallback)) => format_translation(fallback, &args),
                        _ => Text::translate(key, &args, lang),
                    }
                } else if let Some(key) = o.get("keybind").and_then(Value::as_str) {
                    Text::literal(key)
                } else {
                    Text::default()
                };
                let extra = o.get("extra").and_then(Value::as_array).into_iter().flatten();
                extra.fold(content.with_style(&style), |t, e| t.append(Text::from_json(e, lang).with_style(&style)))
            }
            Value::Null => Text::default(),
        }
    }
}

fn object_style(o: &serde_json::Map<String, Value>) -> Style {
    let flag = |k: &str| o.get(k).and_then(Value::as_bool);
    let font = o.get("font").and_then(Value::as_str).map(|f| if f.contains(':') { f.to_owned() } else { format!("minecraft:{f}") });
    let shadow_color = o.get("shadow_color").and_then(|c| match c {
        Value::Number(n) => n.as_i64().map(|v| v as u32),
        _ => None,
    });
    Style {
        color: o.get("color").and_then(Value::as_str).and_then(parse_color),
        shadow_color,
        bold: flag("bold"),
        italic: flag("italic"),
        underlined: flag("underlined"),
        strikethrough: flag("strikethrough"),
        obfuscated: flag("obfuscated"),
        font,
    }
}

/// `TranslatableContents.decomposeTemplate`: `%s` takes the next argument, `%n$s` a numbered one,
/// `%%` is a percent sign; a malformed template is shown as it is.
pub fn format_translation(template: &str, args: &[Text]) -> Text {
    let mut out = Text::default();
    let mut literal = String::new();
    let mut next = 0usize;
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            literal.push(chars[i]);
            i += 1;
            continue;
        }
        let digits = i + 1;
        let mut j = digits;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        let number = if j > digits && chars.get(j) == Some(&'$') {
            let n = chars[digits..j].iter().collect::<String>().parse::<usize>().ok();
            j += 1;
            n
        } else {
            j = digits;
            None
        };
        match chars.get(j).copied() {
            Some('%') if number.is_none() => {
                literal.push('%');
                i = j + 1;
            }
            Some(c) if !c.is_ascii_alphabetic() && c != '%' => {
                literal.push('%');
                i += 1;
            }
            Some('s') => {
                let index = number.map_or_else(|| { next += 1; next - 1 }, |n| n.wrapping_sub(1));
                let Some(arg) = args.get(index) else { return Text::literal(template) };
                if !literal.is_empty() {
                    out.spans.push(Span { text: std::mem::take(&mut literal), style: Style::default() });
                }
                out.spans.extend(arg.spans.iter().cloned());
                i = j + 1;
            }
            _ => return Text::literal(template),
        }
    }
    if !literal.is_empty() {
        out.spans.push(Span { text: literal, style: Style::default() });
    }
    out
}

#[cfg(test)]
mod tests;
