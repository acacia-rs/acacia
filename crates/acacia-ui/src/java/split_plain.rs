//! `StringSplitter` on plain strings (one style, `§` an ordinary character): heads, tails and word
//! positions, as text fields use them. Re-exported by `split`.

use crate::java::gui::Metrics;
use crate::java::text::Style;

/// `StringSplitter.plainIndexAtWidth`: the end of the longest head of `s` at most `max_width` wide.
pub fn plain_index_at_width(m: &dyn Metrics, s: &str, max_width: i32, style: &Style) -> usize {
    let mut left = max_width as f32;
    let mut position = 0;
    for (i, c) in s.char_indices() {
        left -= m.advance(c, style);
        if left < 0.0 {
            break;
        }
        position = i + c.len_utf8();
    }
    position
}

/// `StringSplitter.plainHeadByWidth` (`Font.plainSubstrByWidth`).
pub fn plain_head_by_width<'a>(m: &dyn Metrics, s: &'a str, max_width: i32, style: &Style) -> &'a str {
    &s[..plain_index_at_width(m, s, max_width, style)]
}

/// `StringSplitter.plainTailByWidth` (`Font.plainSubstrByWidth(.., true)`).
pub fn plain_tail_by_width<'a>(m: &dyn Metrics, s: &'a str, max_width: i32, style: &Style) -> &'a str {
    let mut width = 0.0;
    let mut start = s.len();
    for (i, c) in s.char_indices().rev() {
        width += m.advance(c, style);
        if width > max_width as f32 {
            break;
        }
        start = i;
    }
    &s[start..]
}

/// `StringSplitter.getWordPosition`: `dir` words from `from` (negative: back), spaces and
/// newlines separating words, skipped too with `strip_spaces`.
pub fn word_position(text: &str, dir: i32, from: usize, strip_spaces: bool) -> usize {
    let sep = |c: Option<char>| matches!(c, Some(' ' | '\n'));
    let mut result = from;
    for _ in 0..dir.unsigned_abs() {
        if dir < 0 {
            while strip_spaces && result > 0 && sep(text[..result].chars().next_back()) {
                result -= 1;
            }
            while result > 0 && !sep(text[..result].chars().next_back()) {
                result -= text[..result].chars().next_back().map_or(1, char::len_utf8);
            }
        } else {
            result = text[result..].find([' ', '\n']).map_or(text.len(), |i| i + result);
            while strip_spaces && result < text.len() && sep(text[result..].chars().next()) {
                result += 1;
            }
        }
    }
    result
}
