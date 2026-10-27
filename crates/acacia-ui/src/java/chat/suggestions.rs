//! `CommandSuggestions` as the chat screen sets it up (anchored to the bottom, ten lines, never
//! hidden by Tab, messages and commands allowed): a message completes the word at the caret to the
//! online players; a command (`/`) is parsed against the client's command tree for completions,
//! usage hints, errors and the input's colours (`command_info`). Positions are UTF-16 units, as
//! `EditBox`'s. Ported from Horizon's `chat.rs` (MIT) and the game's `CommandSuggestions`.

use std::sync::Arc;

use super::edit_box::EditBox;
use super::suggestions_list::{LINE_LIMIT, SuggestionsList};
use crate::java::command::error::Messages;
use crate::java::command::suggest::{Builder, suggest_values};
use crate::java::command::{Parse, Suggestion, Suggestions, Tree};
use crate::java::gui::{Gui, Metrics};
use crate::java::mth::WHITE;
use crate::java::text::{Lang, Text};

/// `CommandSuggestions`' fill colour in the chat screen (`ChatScreen.USAGE_BACKGROUND_COLOR`).
pub const FILL: u32 = 0xD000_0000;

/// What completes the input.
#[derive(Clone, Debug, Default)]
pub struct Completion {
    /// `getCustomTabSuggestions`: the online players.
    pub names: Vec<String>,
    /// `ClientPacketListener.getCommands()`.
    pub commands: Arc<Tree>,
    pub messages: Messages,
    /// `Options.autoSuggestions`.
    pub auto_suggest: bool,
}

impl Completion {
    /// With `autoSuggestions` on, its default.
    pub fn new(names: Vec<String>, commands: Arc<Tree>, lang: Lang) -> Completion {
        Completion { names, commands, messages: Messages::new(lang), auto_suggest: true }
    }
}

/// What updating the suggestions reads besides the input.
pub struct Sources<'a> {
    pub m: &'a dyn Metrics,
    pub c: &'a Completion,
    pub screen_height: i32,
}

/// `Character.isWhitespace`.
pub fn java_whitespace(c: char) -> bool {
    matches!(c, '\t'..='\r' | '\u{1c}'..='\u{1f}') || (c.is_whitespace() && !matches!(c, '\u{85}' | '\u{a0}' | '\u{2007}' | '\u{202f}'))
}

/// `CommandSuggestions.getLastWordIndex`: after the last run of `\s`.
pub fn last_word_index(s: &[u16]) -> usize {
    let space = |u: u16| matches!(u, 9..=13 | 32);
    (1..=s.len()).rev().find(|&i| space(s[i - 1])).unwrap_or(0)
}

/// Clamp as `Mth.clamp(int)` does: the minimum wins over a smaller maximum.
pub fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo { lo } else { v.min(hi) }
}

#[derive(Clone, Debug, Default)]
pub struct CommandSuggestions {
    /// `pendingSuggestions`: always done here.
    pub(super) pending: Option<Suggestions>,
    pub list: Option<SuggestionsList>,
    pub allow_suggestions: bool,
    pub keep_suggestions: bool,
    /// `currentParse`: the input parsed as a command.
    pub parse: Option<Parse>,
    /// `commandUsage`: usage hints or errors, drawn above the input when no list shows.
    pub usage: Vec<Text>,
    pub(super) usage_x: i32,
    pub(super) usage_width: i32,
}

impl CommandSuggestions {
    /// `updateCommandInfo`.
    pub fn update(&mut self, s: &Sources, input: &mut EditBox) {
        let value: Vec<u16> = input.value().encode_utf16().collect();
        if self.parse.as_ref().is_some_and(|p| p.input != value) {
            self.parse = None;
        }
        if !self.keep_suggestions {
            input.suggestion = None;
            self.list = None;
        }
        self.usage.clear();
        let cursor = input.cursor;
        if value.first() == Some(&u16::from(b'/')) {
            let parse = self.parse.get_or_insert_with(|| s.c.commands.parse(&value, 1)).clone();
            if cursor >= 1 && (self.list.is_none() || !self.keep_suggestions) {
                let found = s.c.commands.completions(&parse, cursor);
                self.pending = Some(found.clone());
                self.update_usage(s, input, &found);
            }
        } else if !input.value().chars().all(java_whitespace) {
            let partial = &value[..cursor];
            self.pending = Some(suggest_values(s.c.names.iter().map(String::as_str), Builder::new(partial, last_word_index(partial))));
            self.usage_width = 0;
            self.usage_x = 0;
        } else {
            self.pending = None;
        }
    }

    /// `setAllowSuggestions`.
    pub fn set_allow(&mut self, allow: bool) {
        self.allow_suggestions = allow;
        if !allow {
            self.list = None;
        }
    }

    /// `showSuggestions`: the list over the input, at the start of what is being completed.
    pub fn show(&mut self, m: &dyn Metrics, input: &mut EditBox, screen_height: i32) {
        let Some(found) = self.pending.as_ref().filter(|p| !p.list.is_empty()) else { return };
        let width = found.list.iter().map(|s| m.width(&Text::legacy(&s.text))).max().unwrap_or(0);
        let start = found.range.map_or(0, |r| r.start);
        let x = clamp(input.screen_x(m, start), 0, input.screen_x(m, 0) + input.inner_width() - width);
        let y = screen_height - 12;
        // sortSuggestions: those starting with the word at the caret first.
        let value: Vec<u16> = input.value().encode_utf16().collect();
        let partial = &value[..input.cursor];
        let word = String::from_utf16_lossy(&partial[last_word_index(partial)..]).to_lowercase();
        let (mut first, rest): (Vec<Suggestion>, Vec<Suggestion>) =
            found.list.iter().cloned().partition(|s| s.text.starts_with(&word) || s.text.starts_with(&format!("minecraft:{word}")));
        first.extend(rest);
        let n = first.len().min(LINE_LIMIT) as i32;
        let left = x - i32::from(!input.bordered);
        self.list = Some(SuggestionsList::new([left, y - 3 - n * 12, width + 1, n * 12], value, first, FILL, input));
    }

    /// `extractRenderState`: the list, or else the usage lines.
    pub fn extract(&mut self, gui: &mut Gui, input: &mut EditBox, mouse: (i32, i32), screen_height: i32) {
        if let Some(list) = self.list.as_mut() {
            list.extract(gui, input, mouse);
            return;
        }
        for (i, line) in self.usage.iter().enumerate() {
            let y = screen_height - 27 - 12 * i as i32;
            gui.fill(self.usage_x - 1, y, self.usage_x + self.usage_width + 1, y + 12, FILL);
            gui.text(line.clone(), self.usage_x, y + 2, WHITE, true);
        }
    }

    /// The input's `TextFormatter` (`formatChat`): `text` from unit `offset` coloured by the
    /// parse, when the input is a command.
    pub fn format(&self, text: &str, offset: usize) -> Option<Text> {
        self.parse.as_ref().map(|p| super::command_info::format_text(p, text, offset))
    }
}
