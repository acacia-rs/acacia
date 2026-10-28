//! The recipe book's widgets as it uses them: the search `EditBox`, `RecipeBookTabButton`, and the
//! hover and `WidgetTooltipHolder` rules of its buttons. Minimal draws of the generic widgets for
//! this module only; the shared widget library lives with the screens.

use super::Tab;
use super::page::squeeze;
use crate::java::container::Env;
use crate::java::container::effects::head_by_width;
use crate::java::gui::{Gui, PoseOp};
use crate::java::mth::WHITE;
use crate::java::text::{Style, Text};
use crate::java::tooltip::{Deferred, Line, Positioner, Tooltip};

/// `AbstractWidget.isHovered` of a widget at `[x, y, w, h]`.
pub fn hovered(rect: [i32; 4], mouse: [i32; 2]) -> bool {
    let [x, y, w, h] = rect;
    mouse[0] >= x && mouse[1] >= y && mouse[0] < x + w && mouse[1] < y + h
}

/// `WidgetTooltipHolder.refreshTooltipForNextRenderPass` of a hovered widget whose delay has passed.
pub fn tooltip(deferred: &mut Deferred, env: &Env, key: &str, rect: [i32; 4], mouse: [i32; 2]) {
    let lines = vec![Line::Text(Text::translate(key, &[], env.lang))];
    deferred.set(Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Menu(rect), style: None, extra_space: false }, false);
}

/// `RecipeBookTabButton.extractContents`: the tab, its icons, squeezed while it has new recipes.
pub fn tab(gui: &mut Gui, tab: &mut Tab, selected: bool, x: i32, y: i32, partial: f32) {
    let anim = tab.anim;
    if anim > 0.0 {
        let (cx, cy) = ((x + 8) as f32, (y + 12) as f32);
        gui.push(vec![PoseOp::Translate(cx, cy), PoseOp::Scale(1.0, squeeze(anim)), PoseOp::Translate(-cx, -cy)]);
    }
    let left = if selected { -2 } else { 0 };
    gui.sprite(if selected { "recipe_book/tab_selected" } else { "recipe_book/tab" }, x + left, y, 35, 27);
    match tab.icons[..] {
        [first, second, ..] => {
            gui.item(first, x + 3 + left, y + 5, 0);
            gui.item(second, x + 14 + left, y + 5, 0);
        }
        [first] => gui.item(first, x + 9 + left, y + 5, 0),
        [] => {}
    }
    if anim > 0.0 {
        gui.pop();
        tab.anim = anim - partial;
    }
}

const SEARCH_WIDTH: i32 = 81;
const SEARCH_HEIGHT: i32 = 14;
const MAX_LENGTH: usize = 50;

/// `EditBox.extractWidgetRenderState` of the bordered, editable search box at `(x, y)`, text colour -1.
pub fn search_box(gui: &mut Gui, env: &Env, s: &super::Search, x: i32, y: i32, _mouse: [i32; 2]) {
    gui.sprite(if s.focused { "widget/text_field_highlighted" } else { "widget/text_field" }, x, y, SEARCH_WIDTH, SEARCH_HEIGHT);
    let (text_x, text_y) = (x + 4, y + (SEARCH_HEIGHT - 8) / 2);
    let chars: Vec<char> = s.value.chars().collect();
    let tail: String = chars[s.display.min(chars.len())..].iter().collect();
    let displayed: Vec<char> = head_by_width(env.font, &Text::literal(tail), (SEARCH_WIDTH - 8) as f32).plain().chars().collect();
    let shown = displayed.len() as i64;
    let rel_cursor = s.cursor as i64 - s.display as i64;
    let on_screen = rel_cursor >= 0 && rel_cursor <= shown;
    let show_cursor = s.focused && (s.blink_ms / 300).is_multiple_of(2) && on_screen;
    let part = |from: usize, to: usize| -> String { displayed[from..to].iter().collect() };
    let mut draw_x = text_x;
    let rel_highlight = (s.highlight as i64 - s.display as i64).clamp(0, shown) as usize;
    if !displayed.is_empty() {
        let half = Text::literal(if on_screen { part(0, rel_cursor as usize) } else { part(0, displayed.len()) });
        let w = env.font.width(&half);
        gui.text(half, draw_x, text_y, WHITE, true);
        draw_x += w + 1;
    }
    let insert = s.cursor < chars.len() || chars.len() >= MAX_LENGTH;
    let mut cursor_x = draw_x;
    if !on_screen {
        cursor_x = if rel_cursor > 0 { text_x + SEARCH_WIDTH } else { text_x };
    } else if insert {
        cursor_x -= 1;
        draw_x -= 1;
    }
    if !displayed.is_empty() && on_screen && rel_cursor < shown {
        gui.text(Text::literal(part(rel_cursor as usize, displayed.len())), draw_x, text_y, WHITE, true);
    }
    if displayed.is_empty() && !s.focused {
        let style = Style { color: Some(0xAAAAAA), italic: Some(true), ..Style::default() };
        gui.text(Text::translate("gui.recipebook.search_hint", &[], env.lang).with_style(&style), draw_x, text_y, WHITE, true);
    }
    if rel_highlight as i64 != rel_cursor {
        let highlight_x = text_x + env.font.width(&Text::literal(part(0, rel_highlight)));
        gui.highlight(cursor_x.min(x + SEARCH_WIDTH), text_y - 1, (highlight_x - 1).min(x + SEARCH_WIDTH), text_y + 1 + 9, true);
    }
    if show_cursor {
        if insert {
            gui.fill(cursor_x, text_y - 1, cursor_x + 1, text_y + 10, WHITE);
        } else {
            gui.text(Text::literal("_"), cursor_x, text_y, WHITE, true);
        }
    }
}
