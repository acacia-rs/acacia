//! `KeyBindsScreen` and its `KeyBindsList`: the mappings sorted under their categories, each with
//! its key's button and a reset button, a stripe on keys bound twice.

use super::Env;
use super::keys::{Category, Mapping, Os, sorted};
use super::listed::{Build, ListedScreen};
use crate::java::text::{Style, Text};
use crate::java::widget::button::Button;
use crate::java::widget::layout::{Elem, Linear};
use crate::java::widget::list::{Entry, SelectionList};
use crate::java::widget::text::{Fill, FocusableTextWidget, StringWidget};
use crate::java::widget::{Ctx, Widget, render};

/// What the key binds screen reads.
#[derive(Clone, Copy, Debug)]
pub struct KeyBinds<'a> {
    /// `options.keyMappings`, in its order (see `keys::defaults`).
    pub mappings: &'a [Mapping],
    /// `selectedKey`: the mapping waiting for a key.
    pub selected: Option<&'a str>,
    pub os: Os,
    /// A frame was drawn before: the reset-all button then shows whether any key differs from its
    /// default (`extractRenderState` sets it after drawing, so the first frame shows it on).
    pub drawn_before: bool,
}

/// The ids the screen's buttons fire: a key's button (`key:` + mapping name), its reset
/// (`reset:` + name), reset all and done.
pub const RESET_ALL: &str = "controls.resetAll";
pub const DONE: &str = "gui.done";

const WHITE: u32 = 0xFF_FFFF;
const YELLOW: u32 = 0xFF_FF55;

/// `updateChangeButtonMessage`.
fn key_message(name: Text, collision: bool, selected: bool) -> Text {
    let white = Style::color(WHITE);
    let mut m = name;
    if collision {
        m = Text::literal("[ ").append(m.with_style(&white)).append(" ]");
    }
    if selected {
        let lit = Style { underlined: Some(true), ..white };
        m = Text::literal("> ").append(m.with_style(&lit)).append(" <");
    }
    if collision || selected { m.with_color(YELLOW) } else { m }
}

/// `KeyBindsList.KeyEntry`.
struct KeyEntry {
    change: Button,
    reset: Button,
    name: Text,
    collision: bool,
}

impl Entry for KeyEntry {
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], _hovered: bool) {
        self.place(row);
        render(&mut self.reset, ctx);
        render(&mut self.change, ctx);
        let [x, y, _, h] = row;
        ctx.gui.text(self.name.clone(), x + 2, y + 2 + (h - 4) / 2 - 9 / 2, 0xFFFF_FFFF, true);
        if self.collision {
            let left = self.change.base.x - 6;
            ctx.gui.fill(left, y + 2 - 1, left + 3, y + 2 + h - 4, 0xFFFF_FF00);
        }
    }
    fn children(&mut self) -> Vec<&mut dyn Widget> {
        vec![&mut self.change as &mut dyn Widget, &mut self.reset]
    }
    fn place(&mut self, [x, y, w, _]: [i32; 4]) {
        let bar = x + w + 6 + 2;
        let reset_x = bar - self.reset.base.w - 10;
        (self.reset.base.x, self.reset.base.y) = (reset_x, y);
        (self.change.base.x, self.change.base.y) = (reset_x - 5 - self.change.base.w, y);
    }
}

/// `KeyBindsList.CategoryEntry`: its label centred on the list, at the bottom of the row.
struct CategoryEntry {
    label: FocusableTextWidget,
    list_width: i32,
}

impl Entry for CategoryEntry {
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], _hovered: bool) {
        self.place(row);
        render(&mut self.label, ctx);
    }
    fn children(&mut self) -> Vec<&mut dyn Widget> {
        vec![&mut self.label as &mut dyn Widget]
    }
    fn place(&mut self, [_, y, _, h]: [i32; 4]) {
        let w = self.label.width();
        self.label.set_x(self.list_width / 2 - w / 2);
        let lh = self.label.height();
        self.label.set_y(y + 2 + h - 4 - lh);
    }
}

/// `KeyEntry.refreshEntry`'s collision: another mapping on the same key, one of them rebound.
fn collides(m: &Mapping, all: &[Mapping]) -> bool {
    !m.is_unbound() && all.iter().any(|o| o.name != m.name && o.key == m.key && (!o.is_default() || !m.is_default()))
}

fn key_entry(ctx: &Ctx, m: &Mapping, k: &KeyBinds) -> KeyEntry {
    let name = ctx.tr(m.name);
    let collision = collides(m, k.mappings);
    let message = key_message(m.key.display(ctx.lang, k.os), collision, k.selected == Some(m.name));
    let mut change = Button::new(0, 0, 75, 20, message);
    change.base.id = format!("key:{}", m.name).into();
    let mut reset = Button::new(0, 0, 50, 20, ctx.tr("controls.reset"));
    reset.base.id = format!("reset:{}", m.name).into();
    reset.base.active = !m.is_default();
    KeyEntry { change, reset, name, collision }
}

/// `KeyBindsList`: 340-wide rows of 20.
fn list(ctx: &Ctx, env: &Env, k: &KeyBinds, h: i32, y: i32) -> SelectionList {
    let mut list = SelectionList::new(env.width, h, y, 340, env.in_world);
    list.selectable = false;
    let mut previous: Option<Category> = None;
    for m in sorted(k.mappings, ctx.lang) {
        if previous != Some(m.category) {
            previous = Some(m.category);
            let text = ctx.tr(m.category.key()).with_style(&Style { bold: Some(true), underlined: Some(true), ..Style::default() });
            let label = FocusableTextWidget::new(ctx.font, text, 4, -1, false, Fill::OnFocus);
            list.push(Box::new(CategoryEntry { label, list_width: env.width }), 20);
        }
        list.push(Box::new(key_entry(ctx, &m, k)), 20);
    }
    list
}

/// `new KeyBindsScreen(lastScreen, options)` after `init`.
pub fn build(ctx: &Ctx, env: Env, k: &KeyBinds) -> ListedScreen {
    let mut b = Build::new(33, 33);
    let title = b.add(StringWidget::fit(ctx.font, ctx.tr("controls.keybinds.title")));
    b.layout.add_header(title);
    let h = b.layout.content_height(env.height);
    b.layout.add_contents(Elem::Spacer { x: 0, y: 0, w: env.width, h });
    let list = list(ctx, &env, k, h, b.layout.header_h);
    let mut reset = Button::named(ctx, 150, RESET_ALL);
    reset.base.active = !k.drawn_before || k.mappings.iter().any(|m| !m.is_default());
    let mut footer = Linear::new(false, 8);
    footer.add_default(b.add(reset));
    footer.add_default(b.add(Button::named(ctx, 150, DONE)));
    b.layout.add_footer(footer.into_elem());
    b.finish(env, vec![(list, 20)])
}
