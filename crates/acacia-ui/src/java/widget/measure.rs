//! `Font` and `StringSplitter` measuring as the widgets use them: widths, the head or tail of a
//! string that fits, and word wrapping. Text is measured with its legacy codes resolved first.
//! The wrapping is ported from Horizon's `screen_draw.rs` (MIT).

use crate::java::gui::Metrics;
use crate::java::text::{Span, Style, Text};

/// `CommonComponents.ELLIPSIS`.
pub const ELLIPSIS: &str = "...";

/// `Font.width(String)`.
pub fn width_str(m: &dyn Metrics, s: &str) -> i32 {
    m.width(&Text::literal(s))
}

/// `StringSplitter.plainIndexAtWidth`: how many characters of `s` fit in `max`, codes drawn as glyphs.
pub fn plain_index_at_width(m: &dyn Metrics, s: &str, max: i32, style: &Style) -> usize {
    let mut left = max as f32;
    let mut position = 0;
    for (i, c) in s.chars().enumerate() {
        left -= m.advance(c, style);
        if left < 0.0 {
            break;
        }
        position = i + 1;
    }
    position
}

/// `Font.plainSubstrByWidth(s, max)`: the head of `s` that fits.
pub fn plain_head(m: &dyn Metrics, s: &str, max: i32) -> String {
    s.chars().take(plain_index_at_width(m, s, max, &Style::default())).collect()
}

/// `Font.plainSubstrByWidth(s, max, true)`: the tail of `s` that fits.
pub fn plain_tail(m: &dyn Metrics, s: &str, max: i32) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut width = 0.0;
    let mut start = chars.len();
    for i in (0..chars.len()).rev() {
        width += m.advance(chars[i], &Style::default());
        if width > max as f32 {
            break;
        }
        start = i;
    }
    chars[start..].iter().collect()
}

/// `StringSplitter.headByWidth` (`Font.substrByWidth`): the leading characters whose advances sum
/// to at most `max`, styles kept.
pub fn head(m: &dyn Metrics, t: &Text, max: i32) -> Text {
    let t = t.resolve_legacy();
    let mut left = max as f32;
    let mut out = Text::default();
    for span in &t.spans {
        let mut taken = String::new();
        for c in span.text.chars() {
            left -= m.advance(c, &span.style);
            if left < 0.0 {
                if !taken.is_empty() {
                    out.spans.push(Span { text: taken, style: span.style.clone() });
                }
                return out;
            }
            taken.push(c);
        }
        if !taken.is_empty() {
            out.spans.push(Span { text: taken, style: span.style.clone() });
        }
    }
    t
}

/// `ComponentRenderUtils.clipText`: as much as fits before an unstyled "...".
pub fn clip(m: &dyn Metrics, t: &Text, max: i32) -> Text {
    head(m, t, max - width_str(m, ELLIPSIS)).append(Text::literal(ELLIPSIS))
}

/// `StringSplitter.splitLines(FormattedText, max, Style.EMPTY)` (`Font.split`): each line breaks at
/// a newline, or at the last space (else the character) once the width passes `max`; the space
/// or newline at a break is dropped and the rest takes the style found there.
pub fn split(m: &dyn Metrics, t: &Text, max: i32) -> Vec<Text> {
    split_wrapped(m, t, max).into_iter().map(|(line, _)| line).collect()
}

/// [`split`] with whether each line continues a wrapped one (`ComponentRenderUtils.wrapComponents`).
pub fn split_wrapped(m: &dyn Metrics, t: &Text, max: i32) -> Vec<(Text, bool)> {
    let max = (max as f32).max(1.0);
    let mut parts: Vec<(Vec<char>, Style)> =
        t.resolve_legacy().spans.into_iter().filter(|s| !s.text.is_empty()).map(|s| (s.text.chars().collect(), s.style)).collect();
    let mut lines = Vec::new();
    let (mut force_new_line, mut wrapped) = (false, false);
    while let Some((at, split_style)) = line_break(m, &parts, max) {
        let flat: Vec<char> = parts.iter().flat_map(|p| p.0.iter().copied()).collect();
        let first = flat[at];
        let new_line = first == '\n';
        let skip = usize::from(new_line || first == ' ');
        force_new_line = new_line;
        let (line, rest) = split_at(std::mem::take(&mut parts), at, skip, &split_style);
        parts = rest;
        lines.push((line, wrapped));
        wrapped = !new_line;
    }
    if !parts.is_empty() {
        let line = Text { spans: parts.into_iter().map(|(c, style)| Span { text: c.into_iter().collect(), style }).collect(), forward: false };
        lines.push((line, wrapped));
    } else if force_new_line {
        lines.push((Text::default(), false));
    }
    lines
}

/// `StringSplitter.LineBreakFinder` over the parts: the break position and style, or `None` when
/// everything fits.
fn line_break(m: &dyn Metrics, parts: &[(Vec<char>, Style)], max: f32) -> Option<(usize, Style)> {
    let (mut width, mut had_visible) = (0.0f32, false);
    let mut last_space: Option<(usize, Style)> = None;
    let mut at = 0;
    for (contents, style) in parts {
        for &c in contents {
            if c == '\n' {
                return Some((at, style.clone()));
            }
            if c == ' ' {
                last_space = Some((at, style.clone()));
            }
            let w = m.advance(c, style);
            width += w;
            if had_visible && width > max {
                return Some(last_space.unwrap_or((at, style.clone())));
            }
            had_visible |= w != 0.0;
            at += 1;
        }
    }
    None
}

/// `FlatComponents.splitAt`: the line before `at` and the parts after the `skip` characters there.
fn split_at(parts: Vec<(Vec<char>, Style)>, at: usize, skip: usize, split_style: &Style) -> (Text, Vec<(Vec<char>, Style)>) {
    let mut line = Text::default();
    let mut rest = Vec::new();
    let mut position = at;
    let (mut in_skip, mut done) = (false, false);
    for (contents, style) in parts {
        if done {
            rest.push((contents, style));
            continue;
        }
        let size = contents.len();
        if !in_skip {
            if position > size {
                line.spans.push(Span { text: contents.iter().collect(), style });
                position -= size;
                continue;
            }
            let before: String = contents[..position].iter().collect();
            if !before.is_empty() {
                line.spans.push(Span { text: before, style: style.clone() });
            }
            position += skip;
            in_skip = true;
        }
        if position <= size {
            if position < size {
                rest.push((contents[position..].to_vec(), split_style.clone()));
            }
            done = true;
            continue;
        }
        position -= size;
    }
    (line, rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::Fixed;

    #[test]
    fn wraps_at_spaces_and_newlines() {
        let m = Fixed(6.0);
        let lines: Vec<String> = split(&m, &Text::literal("ab cd ef\ngh"), 30).iter().map(Text::plain).collect();
        assert_eq!(lines, ["ab cd", "ef", "gh"]);
        assert_eq!(plain_head(&m, "abcdef", 13), "ab");
        assert_eq!(plain_tail(&m, "abcdef", 13), "ef");
        assert_eq!(clip(&m, &Text::literal("abcdefgh"), 30).plain(), "ab...");
    }
}
