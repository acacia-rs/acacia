//! Incoming chat (`Text` packets) as typed messages, plus caller-defined patterns over the plain text.
//!
//! Servers differ in how they send player chat: BDS uses `Chat` with the sender in `source_name`,
//! while proxies such as Geyser often send the formatted line ("[Rank] Name: hi") as `Raw`. Patterns
//! cover the second case: each regex runs against [`ChatMessage::plain`] of every message.

use acacia_client::proto::packets::{Text, TextContent};
use regex::Regex;
use serde_json::Value;

use crate::text::strip_formatting;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    /// A player's chat line.
    Chat,
    Whisper,
    /// `/say` and similar.
    Announcement,
    /// Server text without a sender (most plugin and proxy messages).
    Raw,
    /// A translatable message (`message` is the key, `params` its arguments).
    Translation,
    /// Above the hotbar.
    Popup,
    JukeboxPopup,
    Tip,
    System,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub kind: ChatKind,
    /// The sender's name when the packet names one (`Chat`, `Whisper`, `Announcement`).
    pub sender: Option<String>,
    /// With `§` formatting codes; JSON (rawtext) messages are flattened to their text.
    pub message: String,
    pub params: Vec<String>,
    /// The sender's XUID, when the server sends it.
    pub xuid: String,
}

impl ChatMessage {
    /// `None` for a `Text` packet with no content.
    pub fn from_packet(p: Text) -> Option<Self> {
        use ChatKind as K;
        let (kind, sender, message, params) = match p.content {
            TextContent::Chat(c) => (K::Chat, Some(c.source_name), c.message, Vec::new()),
            TextContent::Whisper(c) => (K::Whisper, Some(c.source_name), c.message, Vec::new()),
            TextContent::Announcement(c) => (K::Announcement, Some(c.source_name), c.message, Vec::new()),
            TextContent::Raw(c) => (K::Raw, None, c.message, Vec::new()),
            TextContent::Tip(c) => (K::Tip, None, c.message, Vec::new()),
            TextContent::System(c) => (K::System, None, c.message, Vec::new()),
            TextContent::Json(c) => (K::Raw, None, rawtext(&c.message), Vec::new()),
            TextContent::JsonWhisper(c) => (K::Whisper, None, rawtext(&c.message), Vec::new()),
            TextContent::JsonAnnouncement(c) => (K::Announcement, None, rawtext(&c.message), Vec::new()),
            TextContent::Translation(c) => (K::Translation, None, c.message, c.parameters),
            TextContent::Popup(c) => (K::Popup, None, c.message, c.parameters),
            TextContent::JukeboxPopup(c) => (K::JukeboxPopup, None, c.message, c.parameters),
            TextContent::Default => return None,
        };
        let sender = sender.filter(|s| !s.is_empty());
        Some(Self { kind, sender, message, params, xuid: p.xuid })
    }

    /// The message without formatting codes.
    pub fn plain(&self) -> String {
        strip_formatting(&self.message)
    }
}

/// A named regex matched against the plain text of every chat message (see the module docs).
#[derive(Debug, Clone)]
pub struct ChatPattern {
    pub name: String,
    pub regex: Regex,
}

impl ChatPattern {
    pub fn new(name: impl Into<String>, regex: &str) -> Result<Self, regex::Error> {
        Ok(Self { name: name.into(), regex: Regex::new(regex)? })
    }

    /// Capture groups 1.. of the first match (an unmatched optional group is empty).
    pub fn captures(&self, plain: &str) -> Option<Vec<String>> {
        let caps = self.regex.captures(plain)?;
        Some(caps.iter().skip(1).map(|g| g.map_or(String::new(), |m| m.as_str().to_owned())).collect())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMatch {
    /// The [`ChatPattern::name`] that matched.
    pub pattern: String,
    pub groups: Vec<String>,
    pub message: ChatMessage,
}

/// The text of a `{"rawtext":[...]}` message: `text` parts verbatim, `translate` parts as their key
/// with `%s`/`%1`.. filled from `with` (no language files here). Unparseable input is returned as is.
pub(crate) fn rawtext(json: &str) -> String {
    match serde_json::from_str::<Value>(json) {
        Ok(v) => flatten(&v),
        Err(_) => json.to_owned(),
    }
}

/// [`rawtext`] for already-parsed JSON.
pub(crate) fn flatten(v: &Value) -> String {
    match v {
        Value::Object(o) => {
            if let Some(Value::Array(parts)) = o.get("rawtext") {
                return parts.iter().map(flatten).collect();
            }
            if let Some(Value::String(t)) = o.get("text") {
                return t.clone();
            }
            if let Some(Value::String(key)) = o.get("translate") {
                let args: Vec<String> = match o.get("with") {
                    Some(Value::Array(a)) => a.iter().map(|x| x.as_str().map_or_else(|| flatten(x), str::to_owned)).collect(),
                    Some(w @ Value::Object(_)) => match w.get("rawtext") {
                        Some(Value::Array(a)) => a.iter().map(flatten).collect(),
                        _ => Vec::new(),
                    },
                    _ => Vec::new(),
                };
                return fill(key, &args);
            }
            String::new()
        }
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Substitutes `%s` in order and `%1`..`%9` by position.
fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut next = 0;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('s') => {
                chars.next();
                out.push_str(args.get(next).map_or("", String::as_str));
                next += 1;
            }
            Some(d @ '1'..='9') => {
                chars.next();
                let i = d as usize - '1' as usize;
                out.push_str(args.get(i).map_or("", String::as_str));
            }
            _ => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use acacia_client::proto::packets::{TextCategory, TextContentChat, TextContentRaw, TextType};

    use super::*;

    fn text(r#type: TextType, content: TextContent) -> Text {
        Text {
            needs_translation: false,
            category: TextCategory::Authored,
            r#type,
            content,
            xuid: "123".into(),
            platform_chat_id: String::new(),
            has_filtered_message: false,
            filtered_message: None,
        }
    }

    #[test]
    fn player_chat_names_the_sender() {
        let p = text(TextType::Chat, TextContent::Chat(TextContentChat { source_name: "Steve".into(), message: "§ahi".into() }));
        let m = ChatMessage::from_packet(p).unwrap();
        assert_eq!((m.kind, m.sender.as_deref(), m.plain().as_str(), m.xuid.as_str()), (ChatKind::Chat, Some("Steve"), "hi", "123"));
    }

    #[test]
    fn raw_lines_match_patterns() {
        let p = text(TextType::Raw, TextContent::Raw(TextContentRaw { message: "§7[§6VIP§7] §fAlex§7: §fsell me dirt".into() }));
        let m = ChatMessage::from_packet(p).unwrap();
        assert_eq!(m.sender, None);
        let pattern = ChatPattern::new("chat", r"^(?:\[\w+\] )?(\w+): (.*)$").unwrap();
        assert_eq!(pattern.captures(&m.plain()), Some(vec!["Alex".into(), "sell me dirt".into()]));
        assert_eq!(pattern.captures("joined the game"), None);
    }

    #[test]
    fn rawtext_flattens_text_and_translations() {
        assert_eq!(rawtext(r#"{"rawtext":[{"text":"§eHello "},{"text":"world"}]}"#), "§eHello world");
        assert_eq!(rawtext(r#"{"rawtext":[{"translate":"%s joined, %s left","with":["A","B"]}]}"#), "A joined, B left");
        assert_eq!(rawtext(r#"{"rawtext":[{"translate":"%2 then %1","with":{"rawtext":[{"text":"x"},{"text":"y"}]}}]}"#), "y then x");
        assert_eq!(rawtext("not json"), "not json");
        assert_eq!(fill("100%", &[]), "100%");
    }
}
