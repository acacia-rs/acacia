//! The `screens_widgets` harness: one widget per case; the schema is its `cases.py`'s.

use std::borrow::Cow;

use acacia_ui::java::gui::{Gui, GuiDraw};
use acacia_ui::java::text::{Text, json_root_style};
use acacia_ui::java::widget::button::{Button, Checkbox, CycleButton, Display, ImageButton, SpriteIconButton, Sprites};
use acacia_ui::java::widget::edit_box::EditBox;
use acacia_ui::java::widget::slider::Slider;
use acacia_ui::java::widget::text::{Fill, FocusableTextWidget, MultiLineText, MultiLineTextWidget, Overflow, StringWidget};
use acacia_ui::java::widget::{Ctx, Widget, render};
use serde_json::{Map, Value};

use super::{common, flag, float, int, ints, lang, layouts, text};
use crate::Assets;

/// `Util.getMillis()` in the probe's clock; the caret blink only reads differences.
const MILLIS: i64 = 1_000_000_000;

fn sprites(v: &Value) -> Sprites {
    let s: Vec<Cow<'static, str>> = v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| Cow::Owned(s.to_owned())).collect();
    Sprites(s[0].clone(), s[1].clone(), s[2].clone(), s[3].clone())
}

pub fn replay(c: &Value, assets: &Assets, _: &mut Map<String, Value>) -> Vec<GuiDraw> {
    let lang = lang(assets);
    let tr = |k: &str| lang.get(k).cloned();
    let mut gui = Gui::new(int(c, "width", 427), int(c, "height", 240));
    let mouse = ints(&c["mouse"]);
    let mut ctx = Ctx::new(&mut gui, assets.font.as_ref(), &tr, (mouse[0], mouse[1]));
    ctx.millis = MILLIS;
    let scissor = ints(&c["scissor"]);
    if scissor.len() == 4 {
        ctx.enable_scissor(scissor[0], scissor[1], scissor[2], scissor[3]);
    }
    match c["kind"].as_str().unwrap_or_default() {
        "layout" => layouts::layout(c, &mut ctx),
        "list" => layouts::list(c, &mut ctx),
        "tabs" => layouts::tabs(c, &mut ctx, lang),
        _ => {
            if let Some(mut w) = widget(c, &ctx, lang) {
                common(w.base_mut(), c);
                render(w.as_mut(), &mut ctx);
            }
        }
    }
    if scissor.len() == 4 {
        ctx.disable_scissor();
    }
    drop(ctx);
    gui.draws
}

fn widget(c: &Value, ctx: &Ctx, lang: &std::collections::HashMap<String, String>) -> Option<Box<dyn Widget>> {
    let (x, y, w, h) = (int(c, "x", 0), int(c, "y", 0), int(c, "w", 150), int(c, "h", 20));
    let msg = text(&c["message"], lang);
    let m = ctx.font;
    Some(match c["kind"].as_str()? {
        "button" => {
            let mut b = Button::new(x, y, w, h, msg);
            b.highlight = c["highlight"].as_bool();
            Box::new(b)
        }
        "image" => Box::new(ImageButton { base: acacia_ui::java::widget::Base::new(x, y, w, h, msg), sprites: sprites(&c["sprites"]) }),
        "icon" => {
            let size = ints(&c["spriteSize"]);
            let mut b = SpriteIconButton::new(w, h, msg, sprites(&c["sprites"]), [size[0], size[1]], flag(c, "iconOnly", true));
            let offset = ints(&c["spriteOffset"]);
            b.sprite_offset = [offset[0], offset[1]];
            b.base.x = x;
            b.base.y = y;
            b.loading = flag(c, "loading", false);
            Box::new(b)
        }
        "cycle" => {
            let values: Vec<Text> = c["values"].as_array()?.iter().map(|v| text(v, lang)).collect();
            let display = match c["display"].as_str() {
                Some("VALUE") => Display::Value,
                Some("HIDE") => Display::Hide,
                _ => Display::NameAndValue,
            };
            let value = values[int(c, "value", 0) as usize].clone();
            let mut b = CycleButton::new(ctx.lang, x, y, w, h, text(&c["name"], lang), value, display);
            b.sprite = c["sprite"].as_str().map(|s| Cow::Owned(s.to_owned()));
            Box::new(b)
        }
        "checkbox" => {
            let mut b = Checkbox::new(m, x, y, c["maxWidth"].as_i64().map(|v| v as i32), msg, flag(c, "selected", false));
            b.text.ellipsis = json_root_style(&c["message"]);
            Box::new(b)
        }
        "slider" => {
            let mut s = Slider::new(x, y, w, h, msg, c["value"].as_f64().unwrap_or(0.0));
            s.can_change = flag(c, "canChange", false);
            Box::new(s)
        }
        "editbox" => Box::new(edit_box(c, m, x, y, w, h, msg, lang)),
        "string" => {
            let mut s = StringWidget::new(m, x, y, w, h, msg);
            if int(c, "maxWidth", 0) > 0 {
                let overflow = if c["overflow"].as_str() == Some("SCROLLING") { Overflow::Scrolling } else { Overflow::Clamped };
                s = s.with_max_width(int(c, "maxWidth", 0), overflow);
            }
            Box::new(s)
        }
        "multiline" => {
            let mut t = MultiLineText::new(msg);
            t.max_width = c["maxWidth"].as_i64().map(|v| v as i32);
            t.max_rows = c["maxRows"].as_u64().map(|v| v as usize);
            t.centered = flag(c, "centered", false);
            t.ellipsis = json_root_style(&c["message"]);
            (t.x, t.y) = (x, y);
            Box::new(MultiLineTextWidget::new(m, t))
        }
        "focusable" => {
            let fill = match c["fill"].as_str() {
                Some("ON_FOCUS") => Fill::OnFocus,
                Some("NEVER") => Fill::Never,
                _ => Fill::Always,
            };
            let mut f = FocusableTextWidget::new(m, msg, int(c, "padding", 4), int(c, "maxWidth", -1), flag(c, "alwaysBorder", true), fill);
            f.base.x = x;
            f.base.y = y;
            Box::new(f)
        }
        _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn edit_box(c: &Value, m: &dyn acacia_ui::java::gui::Metrics, x: i32, y: i32, w: i32, h: i32, msg: Text, lang: &std::collections::HashMap<String, String>) -> EditBox {
    let mut b = EditBox::new(x, y, w, h, msg);
    b.max_length = int(c, "maxLength", 32) as usize;
    b.bordered = flag(c, "bordered", true);
    b.editable = flag(c, "editable", true);
    b.centered = flag(c, "centered", false);
    b.shadow = flag(c, "shadow", true);
    b.invert_highlight = flag(c, "invert", true);
    if let Some(color) = c["textColor"].as_i64() {
        b.text_color = color as u32;
    }
    b.set_value(m, c["value"].as_str().unwrap_or_default());
    for op in c["ops"].as_array().into_iter().flatten() {
        let pos = op[1].as_u64().unwrap_or(0) as usize;
        if op[0] == "cursor" {
            b.move_cursor_to(m, pos, op[2].as_bool().unwrap_or(false));
        } else {
            b.set_highlight_pos(m, pos);
        }
    }
    b.suggestion = c["suggestion"].as_str().map(str::to_owned);
    if !c["hint"].is_null() {
        b.set_hint(text(&c["hint"], lang));
    }
    b.focused_time = MILLIS - i64::from(int(c, "focusAgo", 0));
    let _ = float;
    b
}
