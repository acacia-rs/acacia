use acacia_client::proto::packets::{BookEdit, BookEditContent, BookEditType};

use acacia_client::proto::nbt::Value;

use super::{check_edit, edited_book, has_pages, page_packet, sign_packet, signed_book, with_pages, PageEdit, MAX_PAGE_CHARS};
use crate::state::ItemStack;
use crate::state::queries::test_support::raw;

fn round_trip(packet: &BookEdit) -> BookEdit {
    let decoded: BookEdit = raw(packet).decode().unwrap();
    assert_eq!(&decoded, packet);
    decoded
}

#[test]
fn page_edits_encode() {
    let p = round_trip(&page_packet(3, &PageEdit::Replace { page: 1, text: "line one\nline two".into() }));
    assert_eq!((p.inventory_slot, p.r#type), (3, BookEditType::ReplacePage));
    let BookEditContent::ReplacePage(c) = p.content else { panic!() };
    assert_eq!((c.page_number, c.text.as_str(), c.photo_name.as_str()), (1, "line one\nline two", ""));

    let p = round_trip(&page_packet(0, &PageEdit::Insert { page: 0, text: String::new() }));
    assert_eq!(p.r#type, BookEditType::AddPage);
    let p = round_trip(&page_packet(0, &PageEdit::Delete { page: 4 }));
    assert!(matches!(p.content, BookEditContent::DeletePage(ref c) if c.page_number == 4));
    let p = round_trip(&page_packet(8, &PageEdit::Swap { a: 2, b: 5 }));
    assert!(matches!(p.content, BookEditContent::SwapPages(ref c) if (c.page1, c.page2) == (2, 5)));
}

#[test]
fn signing_encodes_title_author_and_xuid() {
    let p = round_trip(&sign_packet(2, "Diary", "Steve", "2535400000000001"));
    assert_eq!((p.inventory_slot, p.r#type), (2, BookEditType::Sign));
    let BookEditContent::Sign(c) = p.content else { panic!() };
    assert_eq!((c.title.as_str(), c.author.as_str(), c.xuid.as_str()), ("Diary", "Steve", "2535400000000001"));
}

#[test]
fn limits() {
    assert!(check_edit(&PageEdit::Replace { page: 49, text: "x".repeat(MAX_PAGE_CHARS) }).is_ok());
    assert!(check_edit(&PageEdit::Replace { page: 50, text: String::new() }).is_err());
    assert!(check_edit(&PageEdit::Replace { page: 0, text: "x".repeat(MAX_PAGE_CHARS + 1) }).is_err());
    assert!(check_edit(&PageEdit::Swap { a: 0, b: 50 }).is_err());
}

#[test]
fn signing_turns_the_held_book_into_a_written_book() {
    let book = ItemStack { network_id: 7, count: 1, stack_network_id: Some(40), ..ItemStack::default() };
    let signed = signed_book(&book, 8, "Notes", "Steve", "123");
    assert_eq!((signed.network_id, signed.stack_network_id), (8, Some(40)));
    let nbt = signed.nbt.unwrap().value;
    assert_eq!(nbt.get("title"), Some(&Value::String("Notes".into())));
    assert_eq!(nbt.get("author"), Some(&Value::String("Steve".into())));
}

#[test]
fn opened_and_written_books_match_vanilla_nbt() {
    let fresh = ItemStack { network_id: 521, count: 1, ..ItemStack::default() };
    assert!(!has_pages(&fresh));
    let opened = with_pages(&fresh, Vec::new());
    assert!(has_pages(&opened));
    let Some(Value::List(empty)) = opened.nbt.as_ref().unwrap().value.get("pages") else { panic!() };
    assert_eq!((empty.tag, empty.items.len()), (0, 0), "a new book opens with `pages []` of type 0");
    let written = edited_book(&opened, &PageEdit::Replace { page: 0, text: "ass".into() });
    let Some(Value::List(pages)) = written.nbt.as_ref().unwrap().value.get("pages") else { panic!() };
    assert_eq!((pages.tag, pages.items.len()), (10, 1));
    assert_eq!(pages.items[0].get("photoname"), Some(&Value::String("".into())));
}

#[test]
fn page_edits_apply_to_the_held_book() {
    let text = |book: &ItemStack| -> Vec<String> {
        let Some(Value::List(pages)) = book.nbt.as_ref().unwrap().value.get("pages") else { panic!() };
        pages.items.iter().map(|p| match p.get("text") { Some(Value::String(t)) => t.to_string(), _ => panic!() }).collect()
    };
    let mut book = ItemStack { network_id: 7, count: 1, ..ItemStack::default() };
    book = edited_book(&book, &PageEdit::Replace { page: 1, text: "two".into() });
    assert_eq!(text(&book), ["", "two"], "BDS keeps pages as {{photoname, text}}");
    book = edited_book(&book, &PageEdit::Replace { page: 0, text: "one".into() });
    book = edited_book(&book, &PageEdit::Insert { page: 1, text: "mid".into() });
    book = edited_book(&book, &PageEdit::Swap { a: 0, b: 2 });
    book = edited_book(&book, &PageEdit::Delete { page: 1 });
    assert_eq!(text(&book), ["two", "one"]);
}
