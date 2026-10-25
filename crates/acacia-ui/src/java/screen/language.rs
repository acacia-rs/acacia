//! `LanguageSelectScreen`: a search box over the languages' list, the accuracy warning and the
//! Font and Done buttons.

use super::Env;
use super::listed::{Build, ListedScreen};
use crate::java::text::{Style, Text};
use crate::java::widget::button::Button;
use crate::java::widget::edit_box::EditBox;
use crate::java::widget::input::{Action, Fired};
use crate::java::widget::layout::{Elem, Linear};
use crate::java::widget::list::{Entry, SelectionList};
use crate::java::widget::text::StringWidget;
use crate::java::widget::Ctx;

/// `LanguageInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub code: String,
    pub region: String,
    pub name: String,
}

impl Language {
    /// `toComponent`.
    pub fn text(&self) -> Text {
        Text::literal(format!("{} ({})", self.name, self.region))
    }
}

/// What the language screen reads.
#[derive(Clone, Copy, Debug)]
pub struct Languages<'a> {
    /// `LanguageManager.getLanguages()`: any order (shown sorted by code).
    pub languages: &'a [Language],
    /// `getSelected()`, or the entry clicked since the screen opened.
    pub selected: &'a str,
    /// The search box's text.
    pub search: &'a str,
    /// The search box has the focus (as when the screen opens), focused at `focused_time`.
    pub search_focused: bool,
    pub focused_time: i64,
    /// The list has the focus instead (on its selected entry; lost when the search drops it).
    pub list_focused: bool,
    /// The list scrolled to this before the search applied (else where the selection put it).
    pub scroll: Option<f64>,
}

/// The ids: a language entry fires `lang:` + its code; the footer's buttons theirs.
pub const FONT: &str = "options.font";
pub const DONE: &str = "gui.done";
/// The search box's id (the client types into it).
pub const SEARCH: &str = "search";

/// The languages sorted by code (`TreeMap`), kept when `search` is empty or in a name or region
/// (`filterEntries`, lower-cased).
pub fn filtered<'a>(languages: &'a [Language], search: &str) -> Vec<&'a Language> {
    let mut out: Vec<&Language> = languages.iter().collect();
    out.sort_by(|a, b| a.code.encode_utf16().cmp(b.code.encode_utf16()));
    let f = search.to_lowercase();
    out.retain(|l| f.is_empty() || l.name.to_lowercase().contains(&f) || l.region.to_lowercase().contains(&f));
    out
}

/// `LanguageSelectionList.Entry`: the name centred on the list.
struct LanguageEntry {
    code: String,
    text: Text,
    list_width: i32,
}

impl Entry for LanguageEntry {
    fn extract_content(&mut self, ctx: &mut Ctx, [_, y, _, h]: [i32; 4], _hovered: bool) {
        ctx.centered_text(self.text.clone(), self.list_width / 2, y + 2 + (h - 4) / 2 - 9 / 2, 0xFFFF_FFFF);
    }
    fn clicked(&mut self, _x: f64, _y: f64) -> Option<Fired> {
        Some(Fired { id: format!("lang:{}", self.code).into(), action: Action::Press })
    }
}

fn entries(list: &mut SelectionList, langs: &[&Language], width: i32) {
    for l in langs {
        list.push(Box::new(LanguageEntry { code: l.code.clone(), text: l.text(), list_width: width }), 18);
    }
}

/// `new LanguageSelectScreen(lastScreen, options, languageManager)` after `init`, the search box
/// holding `search`.
pub fn build(ctx: &Ctx, env: Env, l: &Languages) -> ListedScreen {
    let mut b = Build::new(33, 53);
    let mut header = Linear::new(true, 4);
    header.grid.defaults = header.grid.defaults.align_x(0.5);
    header.add_default(b.add(StringWidget::fit(ctx.font, ctx.tr("options.language.title"))));
    let mut search = EditBox::new(0, 0, 200, 15, Text::default());
    let hint = Style { color: Some(0xAA_AAAA), italic: Some(true), ..Style::default() };
    search.set_hint(ctx.tr("gui.language.search").with_style(&hint));
    search.set_value(ctx.font, l.search);
    search.base.focused = l.search_focused;
    search.focused_time = l.focused_time;
    search.base.id = SEARCH.into();
    header.add_default(b.add(search));
    b.layout.add_header(header.into_elem());
    b.layout.header_h = 12 + 9 + 15;
    // The list is made at its constructor's size (33 under the top, 53 above the bottom), where it
    // centres the selected language, before `repositionElements` sizes it to the layout.
    let mut list = SelectionList::new(env.width, env.height - 33 - 53, 33, 220 + 50, env.in_world);
    let all = filtered(l.languages, "");
    entries(&mut list, &all, env.width);
    if let Some(i) = all.iter().position(|x| x.code == l.selected) {
        list.set_selected(i);
        list.center_scroll_on(i);
    }
    let h = b.layout.content_height(env.height);
    b.layout.add_contents(Elem::Spacer { x: 0, y: 0, w: env.width, h: env.height - 33 - 53 });
    let mut footer = Linear::new(true, 8);
    footer.grid.defaults = footer.grid.defaults.align_x(0.5);
    footer.add_default(b.add(StringWidget::fit(ctx.font, ctx.tr("options.languageAccuracyWarning").with_color(0xBA_BABA))));
    let mut buttons = Linear::new(false, 8);
    buttons.add_default(b.add(Button::named(ctx, 150, FONT)));
    buttons.add_default(b.add(Button::named(ctx, 150, DONE)));
    footer.add_default(buttons.into_elem());
    b.layout.add_footer(footer.into_elem());
    let header_h = b.layout.header_h;
    let mut screen = b.finish(env, vec![(list, 18)]);
    let list = &mut screen.lists[0];
    (list.base.y, list.base.h) = (header_h, h);
    if let Some(i) = list.selected {
        list.scroll_to_row(i);
    }
    list.set_scroll(l.scroll.unwrap_or(list.scroll));
    if !l.search.is_empty() {
        // The search box's responder: `replaceEntries` drops the selection.
        list.rows.clear();
        list.selected = None;
        entries(list, &filtered(l.languages, l.search), env.width);
        list.set_scroll(list.scroll);
    }
    // Focus lands on the selected entry, as a click on it does; with no selection nothing takes it.
    if l.list_focused && let Some(i) = list.selected {
        list.focused = true;
        list.set_selected(i);
    }
    screen
}
