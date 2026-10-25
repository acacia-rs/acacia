//! `StringSplitter` and `ComponentRenderUtils` over [`Metrics`]: styled text measured, cut and
//! wrapped by width as Java 26.3 does it. Positions are byte offsets into a `&str` (Java's are
//! UTF-16 indices; both name the same characters). Lines come back as [`Text`] whose contents may
//! still hold legacy `§` codes, as Java's `FormattedText` lines do, except where a function says it
//! returns drawn text (`wrap_components`, `split`). Ported from Horizon's `font.rs` (MIT) and
//! checked against the game's `StringSplitter`.

use super::gui::Metrics;
use super::text::{Span, Style, Text};

#[path = "split_plain.rs"]
mod plain;
pub use plain::{plain_head_by_width, plain_index_at_width, plain_tail_by_width, word_position};

/// `StringDecomposer.iterateFormatted(string, 0, current, reset, sink)`: each character with its
/// byte position and the style legacy codes give it (`§r` returns to `reset`, an unknown code is
/// dropped); stops when `sink` returns false. True when it ran to the end.
pub fn iterate_formatted(s: &str, current: &Style, reset: &Style, mut sink: impl FnMut(usize, &Style, char) -> bool) -> bool {
    let mut style = current.clone();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        if c != '§' {
            if !sink(i, &style, c) {
                return false;
            }
            continue;
        }
        let Some((_, code)) = chars.next() else { break };
        style = style.legacy(code.to_lowercase().next().unwrap_or(code), reset);
    }
    true
}

/// `text`'s spans with `initial` under them (`FormattedText.visit(.., initialStyle)`), empty ones left out.
fn parts(text: &Text, initial: &Style) -> Vec<Span> {
    let spans = text.spans.iter().filter(|s| !s.text.is_empty());
    spans.map(|s| {
        let mut style = s.style.clone();
        style.inherit(initial);
        Span { text: s.text.clone(), style }
    }).collect()
}

/// `StringSplitter.stringWidth(FormattedText)`: the advances summed, not rounded.
pub fn string_width(m: &dyn Metrics, text: &Text) -> f32 {
    let mut width = 0.0;
    for part in parts(text, &Style::default()) {
        iterate_formatted(&part.text, &part.style, &part.style, |_, style, c| {
            width += m.advance(c, style);
            true
        });
    }
    width
}

/// `StringSplitter.headByWidth` (`Font.substrByWidth`): the styled head at most `width` wide.
pub fn head_by_width(m: &dyn Metrics, text: &Text, width: i32, initial: &Style) -> Text {
    let mut left = width as f32;
    let mut out = Text::default();
    for part in parts(text, initial) {
        // WidthLimitedCharSink: the width left carries over, the position restarts per part.
        let mut position = 0;
        let whole = iterate_formatted(&part.text, &part.style, &part.style, |i, style, c| {
            left -= m.advance(c, style);
            if left < 0.0 {
                return false;
            }
            position = i + c.len_utf8();
            true
        });
        if !whole {
            if position > 0 {
                out.spans.push(Span { text: part.text[..position].to_owned(), style: part.style });
            }
            return out;
        }
        out.spans.push(part);
    }
    text.clone()
}

/// `ComponentRenderUtils.clipText`: the head that fits `width` with `...` after it, drawn.
pub fn clip_text(m: &dyn Metrics, text: &Text, width: i32) -> Text {
    let ellipsis = Text::literal("...");
    head_by_width(m, text, width - m.width(&ellipsis), &Style::default()).append(ellipsis).resolve_legacy()
}

/// `StringSplitter.LineBreakFinder`.
struct LineBreakFinder<'m> {
    m: &'m dyn Metrics,
    max_width: f32,
    line_break: Option<(usize, Style)>,
    had_visible: bool,
    width: f32,
    last_space: Option<(usize, Style)>,
    next_char: usize,
    offset: usize,
}

impl<'m> LineBreakFinder<'m> {
    fn new(m: &'m dyn Metrics, max_width: i32) -> Self {
        let max_width = (max_width as f32).max(1.0);
        LineBreakFinder { m, max_width, line_break: None, had_visible: false, width: 0.0, last_space: None, next_char: 0, offset: 0 }
    }

    fn accept(&mut self, position: usize, style: &Style, c: char) -> bool {
        let at = position + self.offset;
        match c {
            '\n' => return self.finish(at, style.clone()),
            ' ' => self.last_space = Some((at, style.clone())),
            _ => {}
        }
        let advance = self.m.advance(c, style);
        self.width += advance;
        if !self.had_visible || self.width <= self.max_width {
            self.had_visible |= advance != 0.0;
            self.next_char = at + c.len_utf8();
            return true;
        }
        match self.last_space.take() {
            Some((space, s)) => self.finish(space, s),
            None => self.finish(at, style.clone()),
        }
    }

    fn finish(&mut self, at: usize, style: Style) -> bool {
        self.line_break = Some((at, style));
        false
    }

    fn split_position(&self) -> usize {
        self.line_break.as_ref().map_or(self.next_char, |b| b.0)
    }
}

/// `StringSplitter.findLineBreak`: where `input` would break to fit `max_width`.
pub fn find_line_break(m: &dyn Metrics, input: &str, max_width: i32, initial: &Style) -> usize {
    let mut finder = LineBreakFinder::new(m, max_width);
    iterate_formatted(input, initial, initial, |i, s, c| finder.accept(i, s, c));
    finder.split_position()
}

/// `StringSplitter.splitLines(String, ..)`: `(style at the line's start, start, end)` per line;
/// `include_all` keeps the breaking space or newline at the end of its line.
pub fn split_lines_plain(m: &dyn Metrics, input: &str, max_width: i32, initial: &Style, include_all: bool) -> Vec<(Style, usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut style = initial.clone();
    while start < input.len() {
        let mut finder = LineBreakFinder::new(m, max_width);
        if iterate_formatted(&input[start..], &style, initial, |i, s, c| finder.accept(i + start, s, c)) {
            out.push((style, start, input.len()));
            break;
        }
        let at = finder.split_position();
        let next = if matches!(input.as_bytes()[at], b'\n' | b' ') { at + 1 } else { at };
        out.push((style, start, if include_all { next } else { at }));
        start = next;
        style = finder.line_break.map(|b| b.1).unwrap_or_default();
    }
    out
}

/// `StringSplitter.FlatComponents.splitAt`: the parts before `at` taken off as a line, `skip`
/// bytes after it dropped, and the rest of the part holding `at` restyled `split_style`.
fn split_at(parts: &mut Vec<Span>, at: usize, skip: usize, split_style: &Style) -> Text {
    let mut out = Text::default();
    let mut position = at;
    let mut in_skip = false;
    // Every part passed over is taken off, so the one looked at is always the first.
    while let Some(first) = parts.first() {
        let size = first.text.len();
        if !in_skip {
            if position > size {
                out.spans.push(parts.remove(0));
                position -= size;
                continue;
            }
            if position > 0 {
                out.spans.push(Span { text: first.text[..position].to_owned(), style: first.style.clone() });
            }
            position += skip;
            in_skip = true;
        }
        if position <= size {
            let after = parts[0].text[position..].to_owned();
            if after.is_empty() {
                parts.remove(0);
            } else {
                parts[0] = Span { text: after, style: split_style.clone() };
            }
            break;
        }
        parts.remove(0);
        position -= size;
    }
    out
}

/// `StringSplitter.splitLines(FormattedText, maxWidth, initialStyle, consumer)`: each line and
/// whether it continues a wrapped one. A line breaks at a `\n`, or where a character overflows
/// (never the first visible one) at the last space so far, else before that character; the
/// newline or space at the break is dropped.
pub fn split_lines(m: &dyn Metrics, text: &Text, max_width: i32, initial: &Style) -> Vec<(Text, bool)> {
    let mut parts = parts(text, initial);
    let mut flat: String = parts.iter().map(|p| p.text.as_str()).collect();
    let mut out = Vec::new();
    let (mut force_new_line, mut wrapped) = (false, false);
    'restart: loop {
        let mut finder = LineBreakFinder::new(m, max_width);
        for k in 0..parts.len() {
            let part = &parts[k];
            if iterate_formatted(&part.text, &part.style, initial, |i, s, c| finder.accept(i, s, c)) {
                finder.offset += part.text.len();
                continue;
            }
            let (at, style) = finder.line_break.clone().unwrap_or_else(|| (finder.next_char, Style::default()));
            let tail = flat.as_bytes()[at];
            let new_line = tail == b'\n';
            let skip = usize::from(new_line || tail == b' ');
            force_new_line = new_line;
            out.push((split_at(&mut parts, at, skip, &style), wrapped));
            flat.drain(..at + skip);
            wrapped = !new_line;
            continue 'restart;
        }
        break;
    }
    if !parts.is_empty() {
        out.push((Text { spans: parts, forward: false }, wrapped));
    } else if force_new_line {
        out.push((Text::default(), false));
    }
    out
}

/// `Font.split`: the lines of `text` at most `max_width` wide, drawn (codes resolved).
pub fn split(m: &dyn Metrics, text: &Text, max_width: i32) -> Vec<Text> {
    split_lines(m, text, max_width, &Style::default()).into_iter().map(|(line, _)| line.resolve_legacy()).collect()
}

/// `ComponentRenderUtils.wrapComponents`: the lines drawn, a wrapped one after an unstyled space;
/// one empty line for empty text. `chat_colors` off (`Options.chatColors`) strips legacy codes first.
pub fn wrap_components(m: &dyn Metrics, text: &Text, max_width: i32, chat_colors: bool) -> Vec<Text> {
    let text = if chat_colors { text.clone() } else { text.strip_legacy() };
    let lines: Vec<Text> = split_lines(m, &text, max_width, &Style::default())
        .into_iter()
        .map(|(line, wrapped)| {
            let line = line.resolve_legacy();
            if wrapped { Text::literal(" ").append(line) } else { line }
        })
        .collect();
    if lines.is_empty() { vec![Text::default()] } else { lines }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
