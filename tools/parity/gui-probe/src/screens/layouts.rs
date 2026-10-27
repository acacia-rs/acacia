//! `screens_widgets` kinds that hold more than one widget: layouts of buttons, a selection list and
//! the tab bar. Node schema: the harness's `cases.py`.

use std::collections::HashMap;

use acacia_ui::java::text::Text;
use acacia_ui::java::widget::button::Button;
use acacia_ui::java::widget::layout::{Arena, Elem, Layout, Linear, RowHelper, Settings, align_in_rect};
use acacia_ui::java::widget::list::{Entry, SelectionList};
use acacia_ui::java::widget::tabs::MenuTabBar;
use acacia_ui::java::widget::{Ctx, render};
use serde_json::Value;

use super::{flag, int, ints, text};

fn settings(base: Settings, c: &Value) -> Settings {
    let (Some(p), Some(a)) = (c["pad"].as_array(), c["align"].as_array()) else { return base };
    let p: Vec<i32> = p.iter().filter_map(Value::as_i64).map(|v| v as i32).collect();
    let a: Vec<f32> = a.iter().filter_map(Value::as_f64).map(|v| v as f32).collect();
    base.padding(p[0], p[1], p[2], p[3]).align_x(a[0]).align_y(a[1])
}

fn elem(c: &Value, ws: &mut Arena) -> Elem {
    if let Some(s) = c["button"].as_array() {
        let label = Text::literal(format!("B{}", ws.len()));
        ws.push(Box::new(Button::new(0, 0, s[0].as_i64().unwrap_or(0) as i32, s[1].as_i64().unwrap_or(0) as i32, label)));
        return Elem::Widget(ws.len() - 1);
    }
    if let Some(s) = c["spacer"].as_array() {
        return Elem::Spacer { x: 0, y: 0, w: s[0].as_i64().unwrap_or(0) as i32, h: s[1].as_i64().unwrap_or(0) as i32 };
    }
    Elem::Layout(Box::new(node(&c["layout"], ws)))
}

fn children(n: &Value) -> &[Value] {
    n["children"].as_array().map_or(&[], Vec::as_slice)
}

fn node(n: &Value, ws: &mut Arena) -> Layout {
    let idx = |c: &Value, k: &str| c[k].as_u64().unwrap_or(0) as usize;
    match n["type"].as_str().unwrap_or_default() {
        "grid" => {
            let mut grid = Layout::grid().spacing(int(n, "rowSpacing", 0), int(n, "colSpacing", 0));
            for c in children(n) {
                let e = elem(c, ws);
                grid.add(e, idx(c, "row"), idx(c, "col"), idx(c, "rows"), idx(c, "cols"), settings(grid.defaults, c));
            }
            grid
        }
        "helper" => {
            let mut grid = Layout::grid().spacing(int(n, "rowSpacing", 0), int(n, "colSpacing", 0));
            let mut helper = RowHelper::new(idx(n, "columns"));
            for c in children(n) {
                let e = elem(c, ws);
                let s = settings(grid.defaults, c);
                helper.add(&mut grid, e, idx(c, "span"), s);
            }
            grid
        }
        "linear" => {
            let mut linear = Linear::new(flag(n, "vertical", false), int(n, "spacing", 0));
            for c in children(n) {
                let e = elem(c, ws);
                linear.add(e, settings(linear.grid.defaults, c));
            }
            linear.grid
        }
        "frame" => {
            let mut frame = Layout::frame(int(n, "minW", 0), int(n, "minH", 0));
            for c in children(n) {
                let e = elem(c, ws);
                frame.add_child(e, settings(frame.defaults, c));
            }
            frame
        }
        _ => {
            let mut equal = Layout::equal(int(n, "w", 0), int(n, "h", 0), flag(n, "vertical", false));
            for c in children(n) {
                let e = elem(c, ws);
                equal.add_child(e, settings(equal.defaults, c));
            }
            equal
        }
    }
}

pub fn layout(c: &Value, ctx: &mut Ctx) {
    let mut ws: Arena = Vec::new();
    let mut root = Elem::Layout(Box::new(node(&c["layout"], &mut ws)));
    if let Elem::Layout(l) = &mut root {
        l.arrange(&mut ws);
    }
    let r = &c["rect"];
    let f = |i: usize| r[i].as_f64().unwrap_or(0.0);
    align_in_rect(&mut root, &mut ws, f(0) as i32, f(1) as i32, f(2) as i32, f(3) as i32, f(4) as f32, f(5) as f32);
    let mut order = Vec::new();
    if let Elem::Layout(l) = &root {
        l.visit_widgets(&mut order);
    }
    for i in order {
        render(ws[i].as_mut(), ctx);
    }
}

/// The oracle's list entry: its label at the content corner, yellow when hovered.
struct Label(Text);

impl Entry for Label {
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], hovered: bool) {
        ctx.gui.text(self.0.clone(), row[0] + 2, row[1] + 2, if hovered { 0xFFFF_FF00 } else { 0xFFFF_FFFF }, true);
    }
}

pub fn list(c: &Value, ctx: &mut Ctx) {
    let r = ints(&c["rect"]);
    let mut list = SelectionList::new(r[2], r[3], r[1], int(c, "rowWidth", 220), flag(c, "inWorld", true));
    list.base.x = r[0];
    for (i, h) in ints(&c["rows"]).into_iter().enumerate() {
        list.push(Box::new(Label(Text::literal(format!("Row {i}")))), h);
    }
    list.set_scroll(c["scroll"].as_f64().unwrap_or(0.0));
    if let Ok(i) = usize::try_from(int(c, "selected", -1)) {
        list.focused = flag(c, "focused", false);
        list.set_selected(i);
    }
    render(&mut list, ctx);
}

pub fn tabs(c: &Value, ctx: &mut Ctx, lang: &HashMap<String, String>) {
    let titles = c["titles"].as_array().into_iter().flatten().map(|t| text(t, lang)).collect();
    let mut bar = MenuTabBar::new(titles, int(c, "selected", 0) as usize, int(c, "width", 427));
    render(&mut bar, ctx);
}
