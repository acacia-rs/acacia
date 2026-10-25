//! `PackSelectionScreen` and its two `TransferableSelectionList`s: the available and selected
//! packs, each row an icon (with the select / unselect / move arrows over it on hover), the title
//! and two lines of description.

use std::borrow::Cow;

use super::Env;
use super::listed::{Build, ListedScreen};
use crate::java::gui::{Blend, GuiDraw};
use crate::java::text::{Style, Text};
use crate::java::widget::button::Button;
use crate::java::widget::edit_box::EditBox;
use crate::java::widget::input::{Action, Fired};
use crate::java::widget::layout::{Elem, Linear};
use crate::java::widget::list::{Entry, ListState, SelectionList};
use crate::java::widget::text::{MultiLineText, MultiLineTextWidget, Overflow, StringWidget};
use crate::java::widget::{Ctx, render};

/// `PackCompatibility`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compat {
    Compatible,
    TooOld,
    TooNew,
    Unknown,
}

impl Compat {
    fn key(self) -> &'static str {
        match self {
            Compat::TooOld => "old",
            Compat::TooNew => "new",
            Compat::Unknown => "unknown",
            Compat::Compatible => "compatible",
        }
    }
}

/// A `PackSelectionModel.Entry`.
#[derive(Clone, Debug)]
pub struct Pack {
    pub id: String,
    pub title: Text,
    /// `getExtendedDescription`, and its root style (a cut line's ellipsis takes it).
    pub description: Text,
    pub description_style: Style,
    /// The icon's texture path under `textures/`, without `.png` (`misc/unknown_pack`).
    pub icon: String,
    pub compat: Compat,
    /// `isSelected`: in the selected list.
    pub selected: bool,
    pub required: bool,
    pub fixed: bool,
    pub can_move_up: bool,
    pub can_move_down: bool,
}

/// What the pack screen reads.
#[derive(Clone, Debug)]
pub struct Packs<'a> {
    pub title: Text,
    pub available: &'a [Pack],
    /// Top first, as the selected list shows them.
    pub selected: &'a [Pack],
    pub search: &'a str,
    pub search_focused: bool,
    pub focused_time: i64,
    /// Each list's scroll, before the focus below scrolls its entry into view.
    pub scroll: [f64; 2],
    /// The list (0 available, 1 selected) holding the focus and its focused pack (by index among
    /// the shown packs); takes the focus from the search box, ignored when out of range.
    pub focus: Option<(usize, usize)>,
}

/// The ids: `pack.select:`, `pack.unselect:`, `pack.up:` or `pack.down:` + the pack's id, and the
/// footer's buttons.
pub const OPEN_FOLDER: &str = "pack.openFolder";
pub const DONE: &str = "gui.done";
pub const SEARCH: &str = "search";

/// `filterEntries`: the packs whose id, title or description holds `search` (lower-cased).
pub fn filtered<'a>(packs: &'a [Pack], search: &str) -> Vec<&'a Pack> {
    let f = search.to_lowercase();
    let blank = search.trim().is_empty();
    packs.iter().filter(|p| blank || [p.id.clone(), p.title.plain(), p.description.plain()].iter().any(|s| s.to_lowercase().contains(&f))).collect()
}

struct HeaderEntry {
    text: Text,
}

impl Entry for HeaderEntry {
    fn extract_content(&mut self, ctx: &mut Ctx, [x, y, w, h]: [i32; 4], _hovered: bool) {
        ctx.centered_text(self.text.clone(), x + w / 2, y + 2 + (h - 4) / 2 - 9 / 2, 0xFFFF_FFFF);
    }
}

struct PackEntry {
    pack: Pack,
    state: ListState,
    content: [i32; 2],
}

impl PackEntry {
    fn hover_overlay(&self) -> bool {
        !self.pack.fixed || !self.pack.required
    }

    fn can_unselect(&self) -> bool {
        self.pack.selected && !self.pack.required
    }

    /// The icon's quadrant actions for a point `rel` from its corner: (action, highlighted).
    fn actions(&self, rel: [i32; 2]) -> Vec<(&'static str, bool)> {
        let [rx, ry] = rel;
        let (icon, left) = ((0..32).contains(&rx) && (0..32).contains(&ry), (0..16).contains(&rx) && (0..32).contains(&ry));
        let (top_right, bottom_right) = ((16..32).contains(&rx) && (0..16).contains(&ry), (16..32).contains(&rx) && (16..32).contains(&ry));
        let p = &self.pack;
        if !p.selected {
            return vec![("select", icon)];
        }
        let mut out = Vec::new();
        if self.can_unselect() {
            out.push(("unselect", left));
        }
        if p.can_move_up {
            out.push(("move_up", top_right));
        }
        if p.can_move_down {
            out.push(("move_down", bottom_right));
        }
        out
    }
}

impl Entry for PackEntry {
    fn list_state(&mut self, s: ListState) {
        self.state = s;
    }
    fn place(&mut self, [x, y, _, _]: [i32; 4]) {
        self.content = [x + 2, y + 2];
    }
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], hovered: bool) {
        self.place(row);
        let [cx, cy] = self.content;
        let (right, bottom) = (cx + row[2] - 4, cy + row[3] - 4);
        let p = &self.pack;
        if p.compat != Compat::Compatible {
            ctx.gui.fill(cx - 1, cy - 1, right + 1, bottom + 1, 0xFF77_0000);
        }
        let icon = GuiDraw::Blit { texture: Cow::Owned(p.icon.clone()), blend: Blend::Textured, x: cx, y: cy, w: 32, h: 32, uv: [0.0, 0.0], src: [32, 32], tex: [32, 32], color: 0xFFFF_FFFF };
        ctx.gui.draws.push(icon);
        let mut name = p.title.clone();
        let mut description = p.description.clone().with_color(0x80_8080);
        let mut ellipsis = p.description_style.clone();
        ellipsis.inherit(&Style::color(0x80_8080));
        if self.hover_overlay() && (hovered || self.state.selected && self.state.focused) {
            ctx.gui.fill(cx, cy, cx + 32, cy + 32, 0xA090_9090);
            if p.compat != Compat::Compatible {
                name = ctx.tr("pack.incompatible");
                description = ctx.tr(&format!("pack.incompatible.{}", p.compat.key())).with_color(0xAA_AAAA);
                ellipsis = Style::color(0xAA_AAAA);
            }
            let rel = [ctx.mouse.0 - cx, ctx.mouse.1 - cy];
            for (action, lit) in self.actions(rel) {
                let sprite = format!("transferable_list/{action}{}", if lit { "_highlighted" } else { "" });
                ctx.gui.sprite(sprite, cx, cy, 32, 32);
            }
        }
        let max = 157 - if self.state.scrollable { 6 } else { 0 };
        let mut name = StringWidget::fit(ctx.font, name).with_max_width(max, Overflow::Clamped);
        (name.base.x, name.base.y) = (cx + 34, cy + 1);
        render(&mut name, ctx);
        let mut text = MultiLineText::new(description);
        (text.max_width, text.max_rows, text.ellipsis) = (Some(max), Some(2), ellipsis);
        let mut description = MultiLineTextWidget::new(ctx.font, text);
        (description.base.x, description.base.y) = (cx + 34, cy + 12);
        render(&mut description, ctx);
    }
    fn clicked(&mut self, x: f64, y: f64) -> Option<Fired> {
        if !self.hover_overlay() {
            return None;
        }
        let rel = [x as i32 - self.content[0], y as i32 - self.content[1]];
        let (action, _) = self.actions(rel).into_iter().find(|a| a.1)?;
        let verb = match action {
            "move_up" => "up",
            "move_down" => "down",
            a => a,
        };
        Some(Fired { id: format!("pack.{verb}:{}", self.pack.id).into(), action: Action::Press })
    }
}

/// `TransferableSelectionList.updateList`: the bold underlined title, then the packs.
fn list(ctx: &Ctx, env: &Env, title: &str, packs: &[&Pack], x: i32, y: i32, h: i32) -> SelectionList {
    let mut list = SelectionList::new(200, h, y, 196, env.in_world);
    list.base.x = x;
    (list.bar_at_right, list.narrow_entries) = (true, true);
    let header = ctx.tr(title).with_style(&Style { bold: Some(true), underlined: Some(true), ..Style::default() });
    list.push(Box::new(HeaderEntry { text: header }), 13);
    for p in packs {
        list.push(Box::new(PackEntry { pack: (*p).clone(), state: ListState::default(), content: [0, 0] }), 36);
    }
    list
}

/// `new PackSelectionScreen(repository, output, packDir, title)` after `init`, the lists filled
/// from the model and filtered by `search`.
pub fn build(ctx: &Ctx, env: Env, p: &Packs) -> ListedScreen {
    let mut b = Build::new(4 + 9 + 4 + 9 + 4 + 15 + 4, 33);
    let mut header = Linear::new(true, 4);
    header.grid.defaults = header.grid.defaults.align_x(0.5);
    header.add_default(b.add(StringWidget::fit(ctx.font, p.title.clone())));
    header.add_default(b.add(StringWidget::fit(ctx.font, ctx.tr("pack.dropInfo").with_color(0xAA_AAAA))));
    let mut search = EditBox::new(0, 0, 200, 15, Text::default());
    let hint = Style { color: Some(0xAA_AAAA), italic: Some(true), ..Style::default() };
    search.set_hint(ctx.tr("gui.packSelection.search").with_style(&hint));
    search.set_value(ctx.font, p.search);
    (search.base.focused, search.focused_time, search.base.id) = (p.search_focused, p.focused_time, SEARCH.into());
    header.add_default(b.add(search));
    b.layout.add_header(header.into_elem());
    for _ in 0..2 {
        b.layout.add_contents(Elem::Spacer { x: 0, y: 0, w: 200, h: env.height - 66 });
    }
    let mut footer = Linear::new(false, 8);
    footer.add_default(b.add(Button::named(ctx, 150, OPEN_FOLDER)));
    footer.add_default(b.add(Button::named(ctx, 150, DONE)));
    b.layout.add_footer(footer.into_elem());
    let (h, y) = (b.layout.content_height(env.height), b.layout.header_h);
    let available = list(ctx, &env, "pack.available.title", &filtered(p.available, p.search), env.width / 2 - 15 - 200, y, h);
    let selected = list(ctx, &env, "pack.selected.title", &filtered(p.selected, p.search), env.width / 2 + 15, y, h);
    let mut screen = b.finish(env, vec![(available, 36), (selected, 36)]);
    for (l, s) in screen.lists.iter_mut().zip(p.scroll) {
        l.set_scroll(s);
    }
    if let Some((i, row)) = p.focus
        && let Some(l) = screen.lists.get_mut(i)
        && row + 1 < l.rows.len()
    {
        l.focused = true;
        l.set_selected(row + 1);
        if let Some(w) = screen.widget(SEARCH) {
            w.base_mut().focused = false;
        }
    }
    screen
}
