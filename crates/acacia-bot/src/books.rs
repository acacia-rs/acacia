//! Writing and signing a held book and quill with `BookEdit`. The client edits the book in its hand
//! and names it by hotbar slot (PowerNukkitX accepts only 0-8). Appending a page needs no AddPage:
//! a ReplacePage past the end creates it (PocketMine: "the client will create pages on its own").
//! Sources and the captured vanilla sequence: docs/research/survival-signs-beds.md §4.

use acacia_client::proto::nbt::{List, Nbt, Value};
use acacia_client::proto::packets::{
    BookEdit, BookEditContent, BookEditContentAddPage, BookEditContentDeletePage, BookEditContentReplacePage,
    BookEditContentSign, BookEditContentSwapPages, BookEditType,
};

use crate::human::{BOOK_SIGN, CLICK};
use crate::interact::legacy::{self, SlotChange};
use crate::interact::wire;
use crate::state::ItemStack;
use crate::{ActionError, Bot};

pub const MAX_PAGES: u8 = 50;
/// PowerNukkitX's limit, the strictest of the servers checked.
pub const MAX_PAGE_CHARS: usize = 256;
pub const MAX_TITLE_CHARS: usize = 16;
const WRITABLE_BOOK: &str = "minecraft:writable_book";
const WRITTEN_BOOK: &str = "minecraft:written_book";
/// What the 1.26.52 Windows client put in `author` when signing (capture 2026-10-02, n=1).
const AUTHOR: &str = "Author Unknown";
const COMPOUND_TAG: u8 = 10;

/// One page edit of a book and quill. Pages count from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageEdit {
    Replace { page: u8, text: String },
    /// Inserts a page before an existing one.
    Insert { page: u8, text: String },
    Delete { page: u8 },
    Swap { a: u8, b: u8 },
}

impl Bot {
    /// The pages of the held written book or book and quill, and whether it can be written in;
    /// `None` holding anything else.
    pub fn held_book_pages(&self) -> Option<(Vec<String>, bool)> {
        let held = self.state.inventory.held();
        let writable = match self.state.items.name(held.network_id) {
            Some(WRITABLE_BOOK) => true,
            Some(WRITTEN_BOOK) => false,
            _ => return None,
        };
        let Some(Value::List(pages)) = held.nbt.as_ref().and_then(|n| n.value.get("pages")) else { return Some((Vec::new(), writable)) };
        let text = |page: &Value| if let Some(Value::String(text)) = page.get("text") { text.to_string() } else { String::new() };
        Some((pages.items.iter().map(text).collect(), writable))
    }

    /// The right-click that opens the held book: vanilla's opening packets for a book and quill,
    /// a plain use of anything else.
    pub fn open_held_book(&mut self) {
        match self.held_book() {
            Ok(slot) => self.open_book(slot),
            Err(_) => self.use_item(),
        }
    }

    /// Sends one page edit for the held book and quill.
    pub fn edit_book(&mut self, edit: &PageEdit) -> Result<(), ActionError> {
        let slot = self.held_book()?;
        check_edit(edit)?;
        self.send_page_edit(slot, edit);
        Ok(())
    }

    /// Opens the held book and quill, types `pages` from the first page on, and closes it. Like
    /// vanilla, the text goes out once on close: the slot rewrite, one ReplacePage per page, and
    /// the held item.
    pub async fn write_book(&mut self, pages: &[&str]) -> Result<(), ActionError> {
        let slot = self.held_book()?;
        if pages.len() > usize::from(MAX_PAGES) {
            return Err(ActionError::NotPossible(format!("a book holds at most {MAX_PAGES} pages")));
        }
        let edits: Vec<PageEdit> = pages.iter().enumerate().map(|(i, t)| PageEdit::Replace { page: i as u8, text: (*t).to_owned() }).collect();
        edits.iter().try_for_each(check_edit)?;
        self.open_book(slot);
        let mut time = self.human.typing(pages.iter().map(|t| t.chars().count()).sum());
        for _ in 1..pages.len() {
            time += self.human.between(CLICK);
        }
        self.pause(time).await?;
        let old = self.state.inventory.main[usize::from(slot)].clone();
        let new = edits.iter().fold(old.clone(), |book, edit| edited_book(&book, edit));
        self.client.send(&legacy::rewrite_slot(None, &SlotChange { slot, old: &old, new: &new }));
        for edit in &edits {
            self.client.send(&page_packet(slot, edit));
        }
        self.state.inventory.main[usize::from(slot)] = new;
        self.equip_now();
        Ok(())
    }

    /// Vanilla's open of a new book and quill: two empty ReplacePages and a ClickAir that gives
    /// the book an empty `pages` list, then the held item. A book with pages gets only the ClickAir
    /// (not captured).
    fn open_book(&mut self, slot: u8) {
        let book = self.state.inventory.main[usize::from(slot)].clone();
        if has_pages(&book) {
            self.use_item();
            return;
        }
        for page in 0..2 {
            self.client.send(&page_packet(slot, &PageEdit::Replace { page, text: String::new() }));
        }
        let opened = with_pages(&book, Vec::new());
        let id = self.reflexes.next_legacy_id();
        let click = wire::click_air(self.hand());
        self.client.send(&legacy::with_slot_change(click, Some(id), &SlotChange { slot, old: &book, new: &opened }));
        self.state.inventory.main[usize::from(slot)] = opened;
        self.equip_now();
    }

    /// Signs the held book and quill with `title`. The held item becomes a written book right away,
    /// as in the game: BDS answers with a full resync, other servers may send nothing. The new
    /// `MobEquipment` follows from `crate::reflex`, as vanilla's comes after the server's resync.
    pub async fn sign_book(&mut self, title: &str) -> Result<(), ActionError> {
        let slot = self.held_book()?;
        if title.trim().is_empty() || title.chars().count() > MAX_TITLE_CHARS {
            return Err(ActionError::NotPossible(format!("a book title has 1 to {MAX_TITLE_CHARS} characters")));
        }
        let typing = self.human.typing(title.chars().count()) + self.human.between(BOOK_SIGN);
        self.pause(typing).await?;
        let xuid = self.client.xuid().to_owned();
        let written = self.state.items.id(WRITTEN_BOOK).ok_or_else(|| ActionError::NotPossible("no written book in the item registry".into()))?;
        let old = self.state.inventory.main[usize::from(slot)].clone();
        let new = signed_book(&old, written, title, AUTHOR, &xuid);
        let id = self.reflexes.next_legacy_id();
        self.client.send(&legacy::rewrite_slot(Some(id), &SlotChange { slot, old: &old, new: &new }));
        self.client.send(&sign_packet(slot, title, AUTHOR, &xuid));
        self.state.inventory.main[usize::from(slot)] = new;
        Ok(())
    }

    /// Sends a page edit and applies it to the held book, as the client does: BDS echoes no page
    /// edits, and it rejects the next click made holding an item that differs from its copy.
    fn send_page_edit(&mut self, slot: u8, edit: &PageEdit) {
        self.client.send(&page_packet(slot, edit));
        let book = &mut self.state.inventory.main[usize::from(slot)];
        *book = edited_book(book, edit);
    }

    /// The hotbar slot of the held book and quill.
    fn held_book(&self) -> Result<u8, ActionError> {
        let inv = &self.state.inventory;
        match self.state.items.name(inv.held().network_id) {
            Some(WRITABLE_BOOK) => Ok(inv.selected_hotbar_slot),
            other => Err(ActionError::NotPossible(format!("holding {other:?}, not a book and quill"))),
        }
    }
}

fn check_edit(edit: &PageEdit) -> Result<(), ActionError> {
    let (pages, text) = match edit {
        PageEdit::Replace { page, text } | PageEdit::Insert { page, text } => ([*page, 0], text.as_str()),
        PageEdit::Delete { page } => ([*page, 0], ""),
        PageEdit::Swap { a, b } => ([*a, *b], ""),
    };
    if pages.iter().any(|&p| p >= MAX_PAGES) {
        return Err(ActionError::NotPossible(format!("pages go from 0 to {}", MAX_PAGES - 1)));
    }
    if text.chars().count() > MAX_PAGE_CHARS {
        return Err(ActionError::NotPossible(format!("a page holds at most {MAX_PAGE_CHARS} characters")));
    }
    Ok(())
}

pub(crate) fn page_packet(slot: u8, edit: &PageEdit) -> BookEdit {
    let page = |p: &u8| i32::from(*p);
    let (r#type, content) = match edit {
        PageEdit::Replace { page: p, text } => (
            BookEditType::ReplacePage,
            BookEditContent::ReplacePage(BookEditContentReplacePage { page_number: page(p), text: text.clone(), photo_name: String::new() }),
        ),
        PageEdit::Insert { page: p, text } => (
            BookEditType::AddPage,
            BookEditContent::AddPage(BookEditContentAddPage { page_number: page(p), text: text.clone(), photo_name: String::new() }),
        ),
        PageEdit::Delete { page: p } => (BookEditType::DeletePage, BookEditContent::DeletePage(BookEditContentDeletePage { page_number: page(p) })),
        PageEdit::Swap { a, b } => {
            (BookEditType::SwapPages, BookEditContent::SwapPages(BookEditContentSwapPages { page1: page(a), page2: page(b) }))
        }
    };
    BookEdit { inventory_slot: i32::from(slot), r#type, content }
}

fn has_pages(book: &ItemStack) -> bool {
    book.nbt.as_ref().is_some_and(|n| n.value.get("pages").is_some())
}

/// `book` with its `pages` list replaced; an empty list has element type 0, as vanilla writes it.
pub(crate) fn with_pages(book: &ItemStack, pages: Vec<Value>) -> ItemStack {
    let mut entries = match book.nbt.as_ref().map(|n| &n.value) {
        Some(Value::Compound(entries)) => entries.clone(),
        _ => Vec::new(),
    };
    let tag = if pages.is_empty() { 0 } else { COMPOUND_TAG };
    entries.retain(|(k, _)| k != "pages");
    entries.push(("pages".into(), Value::List(List { tag, items: pages })));
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    ItemStack { nbt: Some(Nbt { name: String::new(), value: Value::Compound(entries) }), ..book.clone() }
}

/// `book` with `edit` applied to its `pages` list of `{photoname, text}`.
pub(crate) fn edited_book(book: &ItemStack, edit: &PageEdit) -> ItemStack {
    let mut pages = match book.nbt.as_ref().and_then(|n| n.value.get("pages")) {
        Some(Value::List(list)) => list.items.clone(),
        _ => Vec::new(),
    };
    let page = |text: &str| Value::Compound(vec![("photoname".into(), Value::String("".into())), ("text".into(), Value::String(text.into()))]);
    match *edit {
        PageEdit::Replace { page: i, ref text } => {
            let i = usize::from(i);
            if pages.len() <= i {
                pages.resize_with(i + 1, || page(""));
            }
            pages[i] = page(text);
        }
        PageEdit::Insert { page: i, ref text } if usize::from(i) <= pages.len() => pages.insert(usize::from(i), page(text)),
        PageEdit::Delete { page: i } if usize::from(i) < pages.len() => drop(pages.remove(usize::from(i))),
        PageEdit::Swap { a, b } if usize::from(a.max(b)) < pages.len() => pages.swap(usize::from(a), usize::from(b)),
        _ => {}
    }
    with_pages(book, pages)
}

/// The written book `book` becomes when signed: its NBT plus the signature, keys in byte order.
pub(crate) fn signed_book(book: &ItemStack, written_book: i32, title: &str, author: &str, xuid: &str) -> ItemStack {
    let mut entries = match book.nbt.as_ref().map(|n| &n.value) {
        Some(Value::Compound(entries)) => entries.clone(),
        _ => Vec::new(),
    };
    let signature =
        [("author", Value::String(author.into())), ("generation", Value::Int(0)), ("title", Value::String(title.into())), ("xuid", Value::String(xuid.into()))];
    entries.retain(|(k, _)| !signature.iter().any(|(s, _)| s == k));
    entries.extend(signature.map(|(k, v)| (k.into(), v)));
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    ItemStack { network_id: written_book, nbt: Some(Nbt { name: String::new(), value: Value::Compound(entries) }), ..book.clone() }
}

pub(crate) fn sign_packet(slot: u8, title: &str, author: &str, xuid: &str) -> BookEdit {
    let content = BookEditContentSign { title: title.to_owned(), author: author.to_owned(), xuid: xuid.to_owned() };
    BookEdit { inventory_slot: i32::from(slot), r#type: BookEditType::Sign, content: BookEditContent::Sign(content) }
}

#[cfg(test)]
mod tests;
