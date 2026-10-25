use super::component::time_alpha;
use super::*;
use crate::java::encode::encode;
use crate::java::gui::{Fixed, Gui};
use crate::java::text::Text;

#[test]
fn lines_fade_out_over_the_last_20_of_200_ticks() {
    assert_eq!(time_alpha(1000, 1000), 1.0);
    assert_eq!(time_alpha(1180, 1000), 1.0);
    assert!((time_alpha(1190, 1000) - 0.25).abs() < 1e-6);
    assert_eq!(time_alpha(1200, 1000), 0.0);
    // A message from the future is opaque.
    assert_eq!(time_alpha(1000, 1005), 1.0);
}

#[test]
fn messages_wrap_newest_at_the_bottom_with_indicators() {
    let m = Fixed(6.0);
    let mut chat = Chat::new(ChatOptions { width: 0.0, ..ChatOptions::default() });
    // 40 px wide: "aaa bbb" breaks after "aaa".
    chat.add(&m, Text::literal("aaa bbb"), None, 100, false);
    chat.add(&m, Text::literal("c"), Some(Tag::SYSTEM), 100, false);
    let mut gui = Gui::new(320, 240);
    chat.extract(&mut gui, 100, false);
    let v = encode(&gui.draws);
    let texts: Vec<(&str, i64)> = v.iter().filter(|d| d["type"] == "text").map(|d| (d["text"].as_str().unwrap(), d["y"].as_i64().unwrap())).collect();
    assert_eq!(texts, [("aaa", 200 - 18 - 8), (" bbb", 200 - 9 - 8), ("c", 200 - 8)]);
    let last = &v[v.len() - 1];
    assert_eq!((last["x0"].as_i64(), last["x1"].as_i64(), last["color"].as_i64()), (Some(-4), Some(-2), Some(0xFFD0D0D0u32 as i32 as i64)));
}

#[test]
fn the_edit_box_scrolls_to_keep_the_caret_shown() {
    let m = Fixed(6.0);
    let mut input = EditBox::new(0, 0, 30, 12);
    input.bordered = false;
    input.max_length = 256;
    // 30 px shows five characters; typing ten leaves the window on the last five.
    for c in "abcdefghij".chars() {
        input.char_typed(&m, c);
    }
    assert_eq!((input.cursor, input.display), (10, 5));
    input.key(&m, Key::Home, false, false);
    assert_eq!((input.cursor, input.display), (0, 0));
    input.key(&m, Key::End, true, false);
    assert_eq!((input.cursor, input.highlight, input.display), (10, 0, 5));
    // Typing over the selection replaces it.
    input.char_typed(&m, 'z');
    assert_eq!(input.value(), "z");
    assert!(!input.char_typed(&m, '§'));
}

#[test]
fn words_are_deleted_and_skipped_by_spaces() {
    let m = Fixed(6.0);
    let mut input = EditBox::new(0, 0, 300, 12);
    input.max_length = 256;
    input.set_value(&m, "one two  three");
    input.key(&m, Key::Left, false, true);
    assert_eq!(input.cursor, 9);
    input.key(&m, Key::Backspace, false, true);
    assert_eq!(input.value(), "one three");
    input.key(&m, Key::A, false, true);
    assert_eq!((input.cursor, input.highlight), (9, 0));
}

#[test]
fn names_complete_the_word_at_the_caret() {
    use super::suggestions::{matches_sub_str, suggest};
    assert!(matches_sub_str("wa", "sky_walker"));
    assert!(!matches_sub_str("ky", "sky_walker"));
    let input: Vec<u16> = "hi st".encode_utf16().collect();
    let found = suggest(["Steve", "stan", "Alex", "Stone_Age"], &input, 3);
    assert_eq!(found.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["stan", "Steve", "Stone_Age"]);
    assert_eq!(found[1].apply(&input), "hi Steve");
}

#[test]
fn tab_shows_the_list_then_applies_and_cycles() {
    let m = Fixed(6.0);
    let names = vec!["Steve".to_owned(), "Stan".to_owned(), "Alex".to_owned()];
    let mut screen = ChatScreen::new(&m, "", 0, names, 320, 240);
    for c in "hi St".chars() {
        screen.char_typed(&m, c);
    }
    screen.key(&m, &[], Key::Tab, false, false);
    assert_eq!(screen.input.suggestion.as_deref(), Some("an"));
    screen.key(&m, &[], Key::Tab, false, false);
    assert_eq!(screen.input.value(), "hi Stan");
    screen.key(&m, &[], Key::Tab, false, false);
    assert_eq!(screen.input.value(), "hi Steve");
    assert_eq!(screen.key(&m, &[], Key::Escape, false, false), ChatAction::None);
    assert_eq!(screen.key(&m, &[], Key::Escape, false, false), ChatAction::Close);
}

#[test]
fn scrolling_stops_at_the_last_page() {
    let m = Fixed(6.0);
    let mut chat = Chat::default();
    for n in 0..30 {
        chat.add(&m, Text::literal(format!("{n}")), None, 0, false);
    }
    // Focused: 180 px, 20 lines a page.
    chat.scroll_chat(1000, true);
    let mut gui = Gui::new(320, 240);
    chat.extract(&mut gui, 0, true);
    let texts: Vec<_> = encode(&gui.draws).into_iter().filter(|d| d["type"] == "text").map(|d| d["text"].as_str().unwrap().to_owned()).collect();
    assert_eq!(texts.len(), 20);
    assert_eq!((texts[0].as_str(), texts[19].as_str()), ("0", "19"));
}
