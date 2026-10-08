//! The chat log and the line being typed. Lines fade as Java's `ChatComponent` fades them.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::draw::DrawList;
use crate::font::LINE_HEIGHT;
use crate::theme::Theme;

/// Lines kept for scrolling back.
const KEPT: usize = 100;
/// Lines shown while the chat is closed.
const SHOWN_CLOSED: usize = 10;
/// A closed chat shows a line this long, the last second fading out (200 ticks).
const SHOWN_FOR: Duration = Duration::from_secs(10);
const FADE: Duration = Duration::from_secs(1);
/// Sent lines kept for the up arrow.
const HISTORY: usize = 100;

pub struct Line {
    /// With `§` formatting codes.
    pub text: String,
    pub at: Instant,
}

#[derive(Default)]
pub struct Chat {
    /// Newest first.
    lines: VecDeque<Line>,
    /// Being typed; `None` while closed.
    pub input: Option<String>,
    sent: VecDeque<String>,
    /// How far back the up arrow went; 0 is the line being typed.
    recall: usize,
}

impl Chat {
    pub fn push(&mut self, text: String, at: Instant) {
        self.lines.push_front(Line { text, at });
        self.lines.truncate(KEPT);
    }

    pub fn is_open(&self) -> bool {
        self.input.is_some()
    }

    /// Opens the input, with `start` typed (`/` for the command key).
    pub fn open(&mut self, start: &str) {
        self.input = Some(start.to_owned());
        self.recall = 0;
    }

    pub fn close(&mut self) {
        self.input = None;
    }

    pub fn type_text(&mut self, text: &str) {
        if let Some(input) = &mut self.input {
            input.extend(text.chars().filter(|c| !c.is_control()));
        }
    }

    pub fn backspace(&mut self) {
        if let Some(input) = &mut self.input {
            input.pop();
        }
    }

    /// Closes the chat and returns what was typed, if anything.
    pub fn submit(&mut self) -> Option<String> {
        let text = self.input.take()?.trim().to_owned();
        if text.is_empty() {
            return None;
        }
        if self.sent.front() != Some(&text) {
            self.sent.push_front(text.clone());
            self.sent.truncate(HISTORY);
        }
        Some(text)
    }

    /// Up (`older`) and down through what was sent before.
    pub fn recall(&mut self, older: bool) {
        let Some(input) = &mut self.input else { return };
        self.recall = if older { (self.recall + 1).min(self.sent.len()) } else { self.recall.saturating_sub(1) };
        *input = if self.recall == 0 { String::new() } else { self.sent[self.recall - 1].clone() };
    }

    /// The lines to draw, newest first, with their opacity (0 to 1).
    pub fn visible(&self, now: Instant) -> Vec<(&str, f32)> {
        if self.is_open() {
            return self.lines.iter().map(|l| (l.text.as_str(), 1.0)).collect();
        }
        self.lines
            .iter()
            .take(SHOWN_CLOSED)
            .map_while(|l| {
                let left = SHOWN_FOR.checked_sub(now.saturating_duration_since(l.at))?;
                Some((l.text.as_str(), (left.as_secs_f32() / FADE.as_secs_f32()).min(1.0)))
            })
            .collect()
    }
}

/// Text width of the log (Java's default chat width).
const WIDTH: f32 = 320.0;
/// GUI pixels between the window bottom and the newest line's bottom.
const BOTTOM: f32 = 40.0;

/// Draws the log over a translucent background and, while open, the input line.
pub fn draw(list: &mut DrawList, theme: &Theme, chat: &Chat, now: Instant, size: [f32; 2]) {
    let Some(font) = &theme.font else { return };
    let [w, h] = size;
    let white = theme.atlas.white();
    for (i, (text, opacity)) in chat.visible(now).into_iter().enumerate() {
        let bottom = h - BOTTOM - i as f32 * LINE_HEIGHT;
        if bottom < LINE_HEIGHT {
            break;
        }
        list.fill(white, [0.0, bottom - LINE_HEIGHT, WIDTH + 8.0, bottom], [0, 0, 0, (opacity * 127.0) as u8]);
        font.draw(list, text, 4.0, bottom - LINE_HEIGHT + 1.0, 0xFFFFFF, opacity, true);
    }
    if let Some(input) = &chat.input {
        list.fill(white, [2.0, h - 14.0, w - 2.0, h - 2.0], [0, 0, 0, 127]);
        let typed = font.draw(list, input, 4.0, h - 12.0, 0xE0E0E0, 1.0, true);
        font.draw(list, "_", 4.0 + typed, h - 12.0, 0xE0E0E0, 1.0, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_chat_fades_old_lines() {
        let start = Instant::now();
        let mut chat = Chat::default();
        chat.push("old".into(), start);
        chat.push("new".into(), start + Duration::from_secs(5));
        let at = start + Duration::from_millis(9500);
        let shown = chat.visible(at);
        assert_eq!(shown[0], ("new", 1.0));
        assert!((shown[1].1 - 0.5).abs() < 1e-3, "{shown:?}");
        assert_eq!(chat.visible(start + Duration::from_secs(11)).len(), 1);
        chat.open("");
        assert_eq!(chat.visible(start + Duration::from_secs(60)).len(), 2, "open shows everything");
    }

    #[test]
    fn up_arrow_recalls_sent_lines() {
        let mut chat = Chat::default();
        for text in ["hi", "/time set day"] {
            chat.open("");
            chat.type_text(text);
            assert_eq!(chat.submit().as_deref(), Some(text));
        }
        chat.open("");
        chat.recall(true);
        assert_eq!(chat.input.as_deref(), Some("/time set day"));
        chat.recall(true);
        chat.recall(true);
        assert_eq!(chat.input.as_deref(), Some("hi"));
        chat.recall(false);
        chat.recall(false);
        assert_eq!(chat.input.as_deref(), Some(""));
    }
}
