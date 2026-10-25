//! The `StringSplitter` measures the station screens need: plain heads and tails by width (an
//! `EditBox`'s visible run) and `Font.split`'s word wrap (widget and onboarding tooltips).

use crate::java::gui::Metrics;
use crate::java::text::{Style, Text};

/// `plainIndexAtWidth`: how many leading characters of `s` fit in `width`.
pub fn plain_head(font: &dyn Metrics, s: &[char], width: i32) -> usize {
    let style = Style::default();
    let mut left = width as f32;
    for (i, &c) in s.iter().enumerate() {
        left -= font.advance(c, &style);
        if left < 0.0 {
            return i;
        }
    }
    s.len()
}

/// `plainTailByWidth`: how many trailing characters of `s` fit in `width`.
pub fn plain_tail(font: &dyn Metrics, s: &[char], width: i32) -> usize {
    let style = Style::default();
    let mut total = 0.0f32;
    let mut n = 0;
    for &c in s.iter().rev() {
        total += font.advance(c, &style);
        if total > width as f32 {
            break;
        }
        n += 1;
    }
    n
}

/// `LineBreakFinder` over `chars`: where the first line ends, `None` when all of it fits.
fn line_break(font: &dyn Metrics, chars: &[(char, Style)], max: f32) -> Option<usize> {
    let (mut width, mut visible, mut last_space) = (0.0f32, false, None);
    for (p, (c, style)) in chars.iter().enumerate() {
        match c {
            '\n' => return Some(p),
            ' ' => last_space = Some(p),
            _ => {}
        }
        let advance = font.advance(*c, style);
        width += advance;
        if visible && width > max {
            return Some(last_space.unwrap_or(p));
        }
        visible |= advance != 0.0;
    }
    None
}

fn text_of(chars: &[(char, Style)]) -> Text {
    let mut out = Text::default();
    for (c, style) in chars {
        match out.spans.last_mut() {
            Some(span) if span.style == *style => span.text.push(*c),
            _ => out = out.append(Text::styled(c.to_string(), style)),
        }
    }
    out
}

/// `Font.split(text, width)`: lines broken at the last space before the width runs out (or
/// mid-word when there is none) and at newlines; the space or newline broken at is dropped.
pub fn split(font: &dyn Metrics, text: &Text, width: i32) -> Vec<Text> {
    let max = width.max(1) as f32;
    let mut rest: Vec<(char, Style)> = text.resolve_legacy().chars().map(|(c, s)| (c, s.clone())).collect();
    let mut lines = Vec::new();
    while let Some(at) = line_break(font, &rest, max) {
        let skip = usize::from(matches!(rest[at].0, ' ' | '\n'));
        lines.push(text_of(&rest[..at]));
        rest.drain(..at + skip);
    }
    if !rest.is_empty() {
        lines.push(text_of(&rest));
    }
    lines
}
