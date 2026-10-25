use super::book_edit::BookEdit;
use super::sign::{Sign, SignEdit, text_color};
use crate::java::gui::Fixed;
use crate::java::widget::input::Key;
use crate::java::widget::multiline::MultilineField;

#[test]
fn sign_text_dims_unless_glowing() {
    assert_eq!(text_color("black", false), 0xFF00_0000);
    assert_eq!(text_color("white", false), 0xFF66_6666);
    assert_eq!(text_color("red", false), 0xFF66_0000);
    assert_eq!(text_color("red", true), 0xFFFF_0000);
    assert_eq!(text_color("black", true), 0xFF00_0000);
}

#[test]
fn sign_typing_stops_at_the_line_width() {
    let m = Fixed(6.0);
    let sign = Sign { wood: "oak".into(), hanging: true, wall: false, dye: "black".into(), glowing: false };
    let mut edit = SignEdit::new(Default::default(), 0);
    for c in "abcdefghijklmnop".chars() {
        edit.char_typed(&m, &sign, c);
    }
    assert_eq!(edit.lines[0], "abcdefghij");
    edit.key(Key::Up, false);
    assert_eq!((edit.line, edit.cursor), (3, 0));
}

#[test]
fn multiline_wraps_and_keeps_a_trailing_newline_line() {
    let m = Fixed(6.0);
    let mut f = MultilineField::new(&m, 30);
    f.set_value(&m, "aaa bbb ccc\n", false);
    let lines = f.lines().to_vec();
    assert_eq!((lines.len(), lines[0].0, lines[2].1, lines[3]), (4, 0, 11, (12, 12)));
    f.seek(lines[1].0 + 1);
    f.seek_line(&m, 1);
    assert_eq!(f.cursor, lines[2].0 + 1);
    f.seek_point(&m, 100.0, 0.0);
    assert_eq!(f.cursor, lines[0].1);
}

#[test]
fn multiline_line_limit_rejects_overflowing_inserts() {
    let m = Fixed(6.0);
    let mut f = MultilineField::new(&m, 30);
    f.line_limit = Some(2);
    assert!(f.insert_text(&m, "a\nb"));
    assert!(!f.insert_text(&m, "\nc"));
    assert_eq!(f.value, "a\nb");
}

#[test]
fn book_forward_appends_a_page_and_saves_without_trailing_empties() {
    let m = Fixed(6.0);
    let mut book = BookEdit::new(&m, vec!["one".into()], 0);
    book.fire(&m, super::book::NEXT);
    assert_eq!((book.page, book.pages.len()), (1, 2));
    book.char_typed(&m, 'x');
    book.fire(&m, super::book::NEXT);
    assert_eq!(book.saved_pages(), vec!["one".to_owned(), "x".to_owned()]);
}
