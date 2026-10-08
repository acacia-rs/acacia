//! A one-line text being typed: the characters and the caret, capped in length.

use crate::input::Key;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextEdit {
    chars: Vec<char>,
    /// Characters before the caret.
    caret: usize,
    max: usize,
}

impl TextEdit {
    pub fn new(text: &str, max: usize) -> TextEdit {
        let chars: Vec<char> = text.chars().take(max).collect();
        TextEdit { caret: chars.len(), chars, max }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// The text before the caret.
    pub fn before_caret(&self) -> String {
        self.chars[..self.caret].iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Types `text` at the caret; control characters and what passes the cap are dropped.
    pub fn insert(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.chars.len() >= self.max {
                break;
            }
            self.chars.insert(self.caret, c);
            self.caret += 1;
        }
    }

    /// Applies an editing key; false for keys it doesn't use.
    pub fn key(&mut self, key: Key) -> bool {
        match key {
            Key::Backspace if self.caret > 0 => {
                self.caret -= 1;
                self.chars.remove(self.caret);
            }
            Key::Delete if self.caret < self.chars.len() => {
                self.chars.remove(self.caret);
            }
            Key::Left => self.caret = self.caret.saturating_sub(1),
            Key::Right => self.caret = (self.caret + 1).min(self.chars.len()),
            Key::Home => self.caret = 0,
            Key::End => self.caret = self.chars.len(),
            Key::Backspace | Key::Delete => {}
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_at_the_caret_up_to_the_cap() {
        let mut e = TextEdit::new("ac", 4);
        e.key(Key::Left);
        e.insert("b\u{7}");
        assert_eq!((e.text(), e.before_caret()), ("abc".to_owned(), "ab".to_owned()));
        e.key(Key::End);
        e.insert("def");
        assert_eq!(e.text(), "abcd");
        e.key(Key::Home);
        e.key(Key::Delete);
        e.key(Key::Backspace);
        assert_eq!(e.text(), "bcd");
        assert!(!e.key(Key::Tab));
    }
}
