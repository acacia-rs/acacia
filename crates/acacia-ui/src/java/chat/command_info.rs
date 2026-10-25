//! `CommandSuggestions`' command half: `updateUsageInfo` (errors, or the usage of what can follow
//! the caret), `fillNodeUsage` and `formatText` (the input coloured by argument).

use super::edit_box::EditBox;
use super::suggestions::{CommandSuggestions, Sources, clamp};
use crate::java::command::error::Kind;
use crate::java::command::reader::Reader;
use crate::java::command::{Parse, Suggestions};
use crate::java::text::{Span, Style, Text};

const GRAY: u32 = 0xAAAAAA;
const RED: u32 = 0xFF5555;
/// `ARGUMENT_STYLES`: aqua, yellow, green, light purple, gold.
const ARGUMENT_COLORS: [u32; 5] = [0x55FFFF, 0xFFFF55, 0x55FF55, 0xFF55FF, 0xFFAA00];

impl CommandSuggestions {
    /// `updateUsageInfo(currentParse, suggestions)`, then the list shown when suggestions are
    /// allowed and `autoSuggestions` is on.
    pub(super) fn update_usage(&mut self, s: &Sources, input: &mut EditBox, found: &Suggestions) {
        let Some(parse) = self.parse.clone() else { return };
        let lang = |k: &str| s.c.messages.get(k);
        let mut trailing = false;
        if input.cursor == input.len() {
            if found.list.is_empty() && !parse.errors.is_empty() {
                let mut literals = 0;
                for (_, e) in &parse.errors {
                    if matches!(e.kind, Kind::LiteralIncorrect(_)) {
                        literals += 1;
                    } else {
                        self.usage.push(e.display(&lang));
                    }
                }
                if literals > 0 {
                    let r = Reader { string: &parse.input, cursor: parse.cursor };
                    self.usage.push(r.error(Kind::UnknownArgument).display(&lang));
                }
            } else if parse.can_read() {
                trailing = true;
            }
        }
        let (node, start) = parse.context.suggestion_context(input.cursor);
        if self.usage.is_empty() {
            let entries: Vec<Text> = s
                .c
                .commands
                .smart_usage(node)
                .into_iter()
                .filter(|&(child, _)| !s.c.commands.node(child).is_literal())
                .map(|(_, usage)| Text::styled(usage, &Style::color(GRAY)).as_forward())
                .collect();
            if entries.is_empty() && trailing {
                self.usage.extend(parse.parse_error().map(|e| e.display(&lang)));
            }
            self.usage.extend(entries);
        }
        self.usage_width = self.usage.iter().map(|l| s.m.visual_width(l)).max().unwrap_or(0);
        self.usage_x = if self.usage.is_empty() { 0 } else { clamp(input.screen_x(s.m, start), 0, input.screen_x(s.m, 0) + input.inner_width() - self.usage_width) };
        self.list = None;
        if self.allow_suggestions && s.c.auto_suggest {
            self.show(s.m, input, s.screen_height);
        }
    }
}

/// `formatText(currentParse, text, offset)`: grey literals, each argument in the next of five
/// colours, what did not parse red; a sequence drawn as it is.
pub fn format_text(parse: &Parse, text: &str, offset: usize) -> Text {
    let t: Vec<u16> = text.encode_utf16().collect();
    let len = t.len() as i64;
    let offset = offset as i64;
    let mut out = Text::default().as_forward();
    let mut push = |a: i64, b: i64, color: u32| {
        if a < b {
            let piece = String::from_utf16_lossy(&t[a as usize..b as usize]);
            out.spans.push(Span { text: piece, style: Style::color(color) });
        }
    };
    let mut unformatted = 0i64;
    for (i, (_, range)) in parse.context.arguments.iter().enumerate() {
        let start = (range.start as i64 - offset).max(0);
        if start >= len {
            break;
        }
        let end = (range.end as i64 - offset).min(len);
        if end > 0 {
            push(unformatted, start, GRAY);
            push(start.max(unformatted), end, ARGUMENT_COLORS[i % ARGUMENT_COLORS.len()]);
            unformatted = end;
        }
    }
    if parse.can_read() {
        let start = (parse.cursor as i64 - offset).max(0);
        if start < len {
            let end = (start + (parse.input.len() - parse.cursor) as i64).min(len);
            push(unformatted, start, GRAY);
            push(start.max(unformatted), end, RED);
            unformatted = end;
        }
    }
    push(unformatted, len, GRAY);
    out
}
