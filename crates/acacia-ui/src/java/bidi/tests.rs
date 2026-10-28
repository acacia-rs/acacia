use super::runs::Run;
use super::*;
use crate::java::text::Span;

fn order(text: &Text) -> (String, Vec<bool>) {
    let v = visual_order(text);
    (v.iter().map(|c| char::from_u32(c.codepoint).unwrap()).collect(), v.iter().map(|c| c.first).collect())
}

fn plain(s: &str) -> (String, Vec<bool>) {
    order(&Text::literal(s))
}

fn runs(s: &str) -> Vec<Run> {
    Bidi::new(s.encode_utf16().collect(), DEFAULT_LTR).visual_runs()
}

#[test]
fn left_to_right_text_is_one_forward_run() {
    assert_eq!(plain("ab c"), ("ab c".into(), vec![true, false, false, false]));
    assert_eq!(runs("ab c"), vec![Run { start: 0, len: 4, odd: false }]);
}

#[test]
fn hebrew_is_reversed_with_position_zero_last() {
    assert_eq!(plain("אבג"), ("גבא".into(), vec![false, false, true]));
}

#[test]
fn neutrals_between_directions_take_the_paragraph_direction() {
    let (s, first) = plain("abc אבג def");
    assert_eq!(s, "abc גבא def");
    assert_eq!(first, vec![true, false, false, false, false, false, true, true, false, false, false]);
}

#[test]
fn brackets_are_paired_and_mirrored() {
    // N0: the pair takes the paragraph's direction (R); reversed, each bracket is mirrored.
    assert_eq!(plain("א(ב)").0, "(ב)א");
    assert_eq!(plain("א [b] ג").0, "ג [b] א");
}

#[test]
fn european_digits_in_a_right_to_left_paragraph() {
    assert_eq!(plain("א 123").0, "123 א");
    assert_eq!(runs("א 123"), vec![Run { start: 2, len: 3, odd: false }, Run { start: 0, len: 2, odd: true }]);
}

#[test]
fn arabic_indic_digits_without_neutrals_stay_left_to_right() {
    // directionFromFlags: AN with nothing neutral is not mixed, so ICU gives one level-0 run.
    assert_eq!(runs("١٢٣"), vec![Run { start: 0, len: 3, odd: false }]);
    assert_eq!(runs("١٢ ٣").len(), 3);
}

#[test]
fn embeddings_raise_levels_and_controls_follow_their_neighbours() {
    // Latin in a right-to-left embedding is level 2: still left to right.
    assert_eq!(plain("a\u{202B}b c\u{202C}d").0, "a\u{202B}b c\u{202C}d");
    // An override forces R; the RLO takes b's level, the PDF d's.
    assert_eq!(plain("a\u{202E}b c\u{202C}d").0, "ac b\u{202E}\u{202C}d");
}

#[test]
fn every_paragraph_has_its_own_level() {
    assert_eq!(plain("א\nb").0, "\nאb");
    assert_eq!(runs("abc\nאב"), vec![Run { start: 0, len: 4, odd: false }, Run { start: 4, len: 2, odd: true }]);
}

#[test]
fn lam_alef_becomes_one_ligature() {
    assert_eq!(arabic::shape(&[0x644, 0x627]), vec![0xFEFB]);
    // Beh between two joiners: initial, medial and final forms.
    assert_eq!(arabic::shape(&[0x628, 0x628, 0x628]), vec![0xFE91, 0xFE92, 0xFE90]);
}

#[test]
fn styles_stay_indexed_by_the_unshaped_text() {
    let red = Style::color(0xFF5555);
    let text = Text { spans: vec![Span { text: "لا".into(), style: Style::default() }, Span { text: "b".into(), style: red }], forward: false };
    let v = visual_order(&text);
    assert_eq!(v.iter().map(|c| c.codepoint).collect::<Vec<_>>(), vec![u32::from('b'), 0xFEFB]);
    assert!(v.iter().all(|c| c.style == Style::default() && c.first));
}

#[test]
fn forward_text_is_drawn_as_it_is() {
    let red = Style::color(0xFF5555);
    let text = Text { spans: vec![Span { text: "אב§c".into(), style: Style::default() }, Span { text: "ג".into(), style: red.clone() }], forward: true };
    let v = visual_order(&text);
    assert_eq!(v.iter().map(|c| char::from_u32(c.codepoint).unwrap()).collect::<String>(), "אב§cג");
    assert_eq!(v.iter().map(|c| c.first).collect::<Vec<_>>(), vec![true, false, false, false, true]);
    assert_eq!(v[4].style, red);
}

#[test]
fn style_changes_split_pieces_in_both_directions() {
    let red = Style::color(0xFF5555);
    let text = Text { spans: vec![Span { text: "אב".into(), style: Style::default() }, Span { text: "ג".into(), style: red.clone() }], forward: false };
    let v = visual_order(&text);
    assert_eq!(v.iter().map(|c| char::from_u32(c.codepoint).unwrap()).collect::<String>(), "גבא");
    assert_eq!(v.iter().map(|c| c.first).collect::<Vec<_>>(), vec![true, false, true]);
    assert_eq!(v[0].style, red);
}

#[test]
fn the_fast_path_agrees_with_the_full_algorithm() {
    for s in ["Hello, world! (1 + 2) = [3]", "\u{2066}a\u{2069} b\u{202A}c\u{202C}\n\td", "ü€ 中文 \u{200D}x"] {
        let fast = plain(s);
        let units: Vec<u16> = s.encode_utf16().collect();
        let bidi = Bidi::new(units.clone(), DEFAULT_LTR);
        let mut full = Vec::new();
        let default = Style::default();
        let styles = vec![&default; units.len()];
        for run in bidi.visual_runs() {
            substring(&bidi.text, &styles, run.start, run.len, run.odd, &mut full);
        }
        assert_eq!(fast.0, full.iter().map(|c| char::from_u32(c.codepoint).unwrap()).collect::<String>(), "{s:?}");
    }
}
