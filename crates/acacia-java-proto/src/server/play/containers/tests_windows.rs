use super::*;
use crate::server::round_trip;
use crate::{VarInt, Wire};

fn bytes<T: Wire>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    value.write(&mut out);
    out
}

#[test]
fn edit_book_caps_pages_and_title() {
    let book = EditBook { slot: VarInt(3), pages: vec!["a".into(), "bc".into()], title: Some("T".into()) };
    assert_eq!(round_trip(book.clone()), [3, 2, 1, b'a', 2, b'b', b'c', 1, 1, b'T']);
    assert_eq!(round_trip(EditBook { title: None, ..book.clone() })[7], 0);
    let long_title = EditBook { title: Some("t".repeat(MAX_BOOK_TITLE + 1)), ..book.clone() };
    assert!(EditBook::read(&mut &bytes(&long_title)[..]).is_err());
    let many = EditBook { pages: vec![String::new(); MAX_BOOK_PAGES + 1], ..book };
    assert!(EditBook::read(&mut &bytes(&many)[..]).is_err());
}

#[test]
fn a_click_decodes_from_vanilla_bytes() {
    // Container 2, state 7, slot 5, button 1, quick move, one changed slot (empty), carried item 9 x1 hashing nothing.
    let wire = [2, 7, 0, 5, 1, 1, 1, 0, 5, 0, 1, 9, 1, 0, 0];
    let click = ContainerClick::read(&mut &wire[..]).unwrap();
    assert_eq!((click.container, click.state_id, click.slot, click.button, click.input), (2, 7, 5, 1, ContainerInput::QuickMove));
    assert_eq!(click.changed, vec![(5, None)]);
    assert_eq!(click.carried, Some(HashedStack { item: 9, count: 1, added: vec![], removed: vec![] }));
    assert_eq!(bytes(&click), wire);
}
