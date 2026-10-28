use super::*;
use crate::java::gui::Fixed;
use crate::java::text::Style;

const M: Fixed = Fixed(6.0);

fn plain(lines: &[Text]) -> Vec<String> {
    lines.iter().map(Text::plain).collect()
}

#[test]
fn wraps_at_the_last_space_and_indents_continuations() {
    let lines = wrap_components(&M, &Text::literal("aaa bbb ccc"), 40, true);
    assert_eq!(plain(&lines), ["aaa", " bbb", " ccc"]);
    let lines = split_lines(&M, &Text::literal("aaa bbb ccc"), 40, &Style::default());
    assert_eq!(lines.iter().map(|l| l.1).collect::<Vec<_>>(), [false, true, true]);
}

#[test]
fn a_word_too_long_breaks_before_the_overflowing_character() {
    assert_eq!(plain(&split(&M, &Text::literal("abcdefgh"), 20)), ["abc", "def", "gh"]);
    // The first visible character always fits.
    assert_eq!(plain(&split(&M, &Text::literal("abc"), 0)), ["a", "b", "c"]);
}

#[test]
fn newlines_break_and_a_trailing_one_leaves_an_empty_line() {
    assert_eq!(plain(&split(&M, &Text::literal("ab\n"), 100)), ["ab", ""]);
    let lines = split_lines(&M, &Text::literal("\nab\ncd"), 100, &Style::default());
    assert_eq!(lines.iter().map(|l| (l.0.plain(), l.1)).collect::<Vec<_>>(), [("".into(), false), ("ab".into(), false), ("cd".into(), false)]);
    assert_eq!(plain(&wrap_components(&M, &Text::default(), 100, true)), [""]);
}

#[test]
fn styles_carry_across_parts_and_bold_is_wider() {
    let red = Style::color(0xFF5555);
    let bold = Style { bold: Some(true), ..Style::default() };
    let text = Text::styled("aa ", &red).append(Text::styled("bb", &bold));
    let lines = split_lines(&M, &text, 20, &Style::default());
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, Text::styled("aa", &red));
    assert_eq!(lines[1].0, Text::styled("bb", &bold));
    assert_eq!(string_width(&M, &text), 6.0 * 3.0 + 7.0 * 2.0);
}

#[test]
fn legacy_codes_style_the_rest_of_a_split_part() {
    let lines = wrap_components(&M, &Text::literal("§cab cd"), 20, true);
    assert_eq!(lines[0], Text::legacy("§cab"));
    assert_eq!(lines[1], Text::literal(" ").append(Text::legacy("§ccd")));
    // Chat Colors off: the codes are stripped before wrapping.
    assert_eq!(plain(&wrap_components(&M, &Text::literal("§cab cd"), 100, false)), ["ab cd"]);
}

#[test]
fn heads_and_tails_by_width() {
    let s = Style::default();
    assert_eq!(plain_head_by_width(&M, "abcdef", 20, &s), "abc");
    assert_eq!(plain_tail_by_width(&M, "abcdef", 20, &s), "def");
    assert_eq!(plain_head_by_width(&M, "abc", 18, &s), "abc");
    let red = Style::color(0xFF5555);
    let text = Text::literal("ab").append(Text::styled("cdef", &red));
    assert_eq!(head_by_width(&M, &text, 25, &s), Text::literal("ab").append(Text::styled("cd", &red)));
    assert_eq!(head_by_width(&M, &text, 100, &s), text);
    assert_eq!(clip_text(&M, &Text::literal("abcdefgh"), 36).plain(), "abc...");
}

#[test]
fn plain_lines_and_line_breaks() {
    let s = Style::default();
    assert_eq!(find_line_break(&M, "aaa bbb", 40, &s), 3);
    assert_eq!(find_line_break(&M, "aaa", 40, &s), 3);
    let lines = split_lines_plain(&M, "aaa bbb\ncc", 40, &s, false);
    assert_eq!(lines.iter().map(|l| (l.1, l.2)).collect::<Vec<_>>(), [(0, 3), (4, 7), (8, 10)]);
    let lines = split_lines_plain(&M, "aaa bbb", 40, &s, true);
    assert_eq!(lines.iter().map(|l| (l.1, l.2)).collect::<Vec<_>>(), [(0, 4), (4, 7)]);
}

#[test]
fn word_positions() {
    let t = "one two  three";
    assert_eq!(word_position(t, 1, 0, true), 4);
    assert_eq!(word_position(t, 1, 0, false), 3);
    assert_eq!(word_position(t, 2, 0, true), 9);
    assert_eq!(word_position(t, -1, 9, true), 4);
    assert_eq!(word_position(t, -1, t.len(), true), 9);
    assert_eq!(word_position(t, 1, 9, true), t.len());
}
