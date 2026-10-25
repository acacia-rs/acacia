//! `GridLayout`, `LinearLayout`, `FrameLayout` and `EqualSpacingLayout` positioning widgets held in
//! an arena (`Vec<Box<dyn Widget>>`) by index, in Java's integer arithmetic; and
//! `HeaderAndFooterLayout`.

use super::Widget;
use crate::java::mth::lerp;

pub type Arena = Vec<Box<dyn Widget>>;

/// `LayoutSettings`: padding (left, top, right, bottom) and alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings {
    pub pad: [i32; 4],
    pub align: [f32; 2],
}

impl Settings {
    pub fn padding(mut self, left: i32, top: i32, right: i32, bottom: i32) -> Settings {
        self.pad = [left, top, right, bottom];
        self
    }
    pub fn pad_top(mut self, v: i32) -> Settings {
        self.pad[1] = v;
        self
    }
    pub fn pad_bottom(mut self, v: i32) -> Settings {
        self.pad[3] = v;
        self
    }
    pub fn align_x(mut self, v: f32) -> Settings {
        self.align[0] = v;
        self
    }
    pub fn align_y(mut self, v: f32) -> Settings {
        self.align[1] = v;
        self
    }
}

/// A `LayoutElement`.
pub enum Elem {
    Widget(usize),
    Spacer { x: i32, y: i32, w: i32, h: i32 },
    Layout(Box<Layout>),
}

impl Elem {
    pub fn x(&self, ws: &Arena) -> i32 {
        match self {
            Elem::Widget(i) => ws[*i].base().x,
            Elem::Spacer { x, .. } => *x,
            Elem::Layout(l) => l.x,
        }
    }
    pub fn y(&self, ws: &Arena) -> i32 {
        match self {
            Elem::Widget(i) => ws[*i].base().y,
            Elem::Spacer { y, .. } => *y,
            Elem::Layout(l) => l.y,
        }
    }
    pub fn w(&self, ws: &Arena) -> i32 {
        match self {
            Elem::Widget(i) => ws[*i].width(),
            Elem::Spacer { w, .. } => *w,
            Elem::Layout(l) => l.w,
        }
    }
    pub fn h(&self, ws: &Arena) -> i32 {
        match self {
            Elem::Widget(i) => ws[*i].height(),
            Elem::Spacer { h, .. } => *h,
            Elem::Layout(l) => l.h,
        }
    }
    /// `setX`: a layout moves its children by the difference.
    pub fn set_x(&mut self, ws: &mut Arena, v: i32) {
        match self {
            Elem::Widget(i) => ws[*i].set_x(v),
            Elem::Spacer { x, .. } => *x = v,
            Elem::Layout(l) => l.set_x(ws, v),
        }
    }
    pub fn set_y(&mut self, ws: &mut Arena, v: i32) {
        match self {
            Elem::Widget(i) => ws[*i].set_y(v),
            Elem::Spacer { y, .. } => *y = v,
            Elem::Layout(l) => l.set_y(ws, v),
        }
    }
    fn arrange(&mut self, ws: &mut Arena) {
        if let Elem::Layout(l) = self {
            l.arrange(ws);
        }
    }
    fn visit(&self, out: &mut Vec<usize>) {
        match self {
            Elem::Widget(i) => out.push(*i),
            Elem::Spacer { .. } => {}
            Elem::Layout(l) => l.visit_widgets(out),
        }
    }
}

pub struct Cell {
    pub elem: Elem,
    pub settings: Settings,
    pub row: usize,
    pub col: usize,
    pub rows: usize,
    pub cols: usize,
}

impl Cell {
    fn w(&self, ws: &Arena) -> i32 {
        self.elem.w(ws) + self.settings.pad[0] + self.settings.pad[2]
    }
    fn h(&self, ws: &Arena) -> i32 {
        self.elem.h(ws) + self.settings.pad[1] + self.settings.pad[3]
    }
    /// `AbstractChildWrapper.setX`: truncated.
    fn place_x(&mut self, ws: &mut Arena, x: i32, space: i32) {
        let most = (space - self.elem.w(ws) - self.settings.pad[2]) as f32;
        let offset = lerp(self.settings.align[0], self.settings.pad[0] as f32, most) as i32;
        self.elem.set_x(ws, offset + x);
    }
    /// `AbstractChildWrapper.setY`: rounded.
    fn place_y(&mut self, ws: &mut Arena, y: i32, space: i32) {
        let most = (space - self.elem.h(ws) - self.settings.pad[3]) as f32;
        let offset = java_round(lerp(self.settings.align[1], self.settings.pad[1] as f32, most));
        self.elem.set_y(ws, offset + y);
    }
}

/// `Math.round(float)`.
pub fn java_round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Grid { row_spacing: i32, col_spacing: i32 },
    Frame { min_w: i32, min_h: i32 },
    /// `EqualSpacingLayout`; `vertical` its orientation.
    Equal { vertical: bool },
}

pub struct Layout {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub cells: Vec<Cell>,
    /// `defaultCellSetting()`: what new cells start from.
    pub defaults: Settings,
}

impl Layout {
    pub fn grid() -> Layout {
        Layout { kind: Kind::Grid { row_spacing: 0, col_spacing: 0 }, x: 0, y: 0, w: 0, h: 0, cells: Vec::new(), defaults: Settings::default() }
    }

    /// `new FrameLayout(minW, minH)`: children centred.
    pub fn frame(min_w: i32, min_h: i32) -> Layout {
        Layout { kind: Kind::Frame { min_w, min_h }, x: 0, y: 0, w: min_w, h: min_h, cells: Vec::new(), defaults: Settings { pad: [0; 4], align: [0.5, 0.5] } }
    }

    pub fn equal(w: i32, h: i32, vertical: bool) -> Layout {
        Layout { kind: Kind::Equal { vertical }, x: 0, y: 0, w, h, cells: Vec::new(), defaults: Settings::default() }
    }

    pub fn spacing(mut self, rows: i32, cols: i32) -> Layout {
        self.kind = Kind::Grid { row_spacing: rows, col_spacing: cols };
        self
    }

    pub fn add(&mut self, elem: Elem, row: usize, col: usize, rows: usize, cols: usize, settings: Settings) {
        self.cells.push(Cell { elem, settings, row, col, rows, cols });
    }

    /// `FrameLayout.addChild` / `EqualSpacingLayout.addChild`.
    pub fn add_child(&mut self, elem: Elem, settings: Settings) {
        self.add(elem, 0, 0, 1, 1, settings);
    }

    pub fn set_x(&mut self, ws: &mut Arena, x: i32) {
        let d = x - self.x;
        for c in &mut self.cells {
            let cx = c.elem.x(ws);
            c.elem.set_x(ws, cx + d);
        }
        self.x = x;
    }

    pub fn set_y(&mut self, ws: &mut Arena, y: i32) {
        let d = y - self.y;
        for c in &mut self.cells {
            let cy = c.elem.y(ws);
            c.elem.set_y(ws, cy + d);
        }
        self.y = y;
    }

    pub fn visit_widgets(&self, out: &mut Vec<usize>) {
        for c in &self.cells {
            c.elem.visit(out);
        }
    }

    pub fn arrange(&mut self, ws: &mut Arena) {
        for c in &mut self.cells {
            c.elem.arrange(ws);
        }
        match self.kind {
            Kind::Grid { row_spacing, col_spacing } => self.arrange_grid(ws, row_spacing, col_spacing),
            Kind::Frame { min_w, min_h } => {
                let w = self.cells.iter().map(|c| c.w(ws)).fold(min_w, i32::max);
                let h = self.cells.iter().map(|c| c.h(ws)).fold(min_h, i32::max);
                let (x, y) = (self.x, self.y);
                for c in &mut self.cells {
                    c.place_x(ws, x, w);
                    c.place_y(ws, y, h);
                }
                self.w = w;
                self.h = h;
            }
            Kind::Equal { vertical } => self.arrange_equal(ws, vertical),
        }
    }

    fn arrange_grid(&mut self, ws: &mut Arena, row_spacing: i32, col_spacing: i32) {
        let max_row = self.cells.iter().map(|c| c.row + c.rows - 1).max().unwrap_or(0);
        let max_col = self.cells.iter().map(|c| c.col + c.cols - 1).max().unwrap_or(0);
        let mut col_w = vec![0; max_col + 1];
        let mut row_h = vec![0; max_row + 1];
        for c in &self.cells {
            let mut d = Divisor::new(c.h(ws) - (c.rows as i32 - 1) * row_spacing, c.rows as i32);
            for h in row_h.iter_mut().skip(c.row).take(c.rows) {
                *h = (*h).max(d.next());
            }
            let mut d = Divisor::new(c.w(ws) - (c.cols as i32 - 1) * col_spacing, c.cols as i32);
            for w in col_w.iter_mut().skip(c.col).take(c.cols) {
                *w = (*w).max(d.next());
            }
        }
        let mut col_x = vec![0; max_col + 1];
        for i in 1..=max_col {
            col_x[i] = col_x[i - 1] + col_w[i - 1] + col_spacing;
        }
        let mut row_y = vec![0; max_row + 1];
        for i in 1..=max_row {
            row_y[i] = row_y[i - 1] + row_h[i - 1] + row_spacing;
        }
        let (x, y) = (self.x, self.y);
        for c in &mut self.cells {
            let aw = col_w[c.col..c.col + c.cols].iter().sum::<i32>() + col_spacing * (c.cols as i32 - 1);
            c.place_x(ws, x + col_x[c.col], aw);
            let ah = row_h[c.row..c.row + c.rows].iter().sum::<i32>() + row_spacing * (c.rows as i32 - 1);
            c.place_y(ws, y + row_y[c.row], ah);
        }
        self.w = col_x[max_col] + col_w[max_col];
        self.h = row_y[max_row] + row_h[max_row];
    }

    fn arrange_equal(&mut self, ws: &mut Arena, vertical: bool) {
        if self.cells.is_empty() {
            return;
        }
        let primary = |c: &Cell, ws: &Arena| if vertical { c.h(ws) } else { c.w(ws) };
        let secondary = |c: &Cell, ws: &Arena| if vertical { c.w(ws) } else { c.h(ws) };
        let total: i32 = self.cells.iter().map(|c| primary(c, ws)).sum();
        let max_secondary = self.cells.iter().map(|c| secondary(c, ws)).fold(if vertical { self.w } else { self.h }, i32::max);
        let remaining = (if vertical { self.h } else { self.w }) - total;
        let mut position = if vertical { self.y } else { self.x };
        let mut d = Divisor::new(remaining, self.cells.len() as i32 - 1);
        let secondary_pos = if vertical { self.x } else { self.y };
        for (i, c) in self.cells.iter_mut().enumerate() {
            if i > 0 {
                position += d.next();
            }
            let size = primary(c, ws);
            if vertical { c.place_y(ws, position, size) } else { c.place_x(ws, position, size) }
            position += size;
        }
        for c in &mut self.cells {
            if vertical { c.place_x(ws, secondary_pos, max_secondary) } else { c.place_y(ws, secondary_pos, max_secondary) }
        }
        if vertical { self.w = max_secondary } else { self.h = max_secondary }
    }
}

/// `com.mojang.math.Divisor`: `n` in `d` integer parts, the remainder spread evenly.
pub struct Divisor {
    d: i32,
    q: i32,
    m: i32,
    r: i32,
}

impl Divisor {
    pub fn new(n: i32, d: i32) -> Divisor {
        if d > 0 { Divisor { d, q: n / d, m: n % d, r: 0 } } else { Divisor { d, q: 0, m: 0, r: 0 } }
    }
    pub fn next(&mut self) -> i32 {
        let mut v = self.q;
        self.r += self.m;
        if self.r >= self.d {
            self.r -= self.d;
            v += 1;
        }
        v
    }
}

/// `LinearLayout`: a one-row or one-column grid.
pub struct Linear {
    pub grid: Layout,
    pub vertical: bool,
    next: usize,
}

impl Linear {
    pub fn new(vertical: bool, spacing: i32) -> Linear {
        let grid = if vertical { Layout::grid().spacing(spacing, 0) } else { Layout::grid().spacing(0, spacing) };
        Linear { grid, vertical, next: 0 }
    }
    pub fn add(&mut self, elem: Elem, settings: Settings) {
        let i = self.next;
        self.next += 1;
        if self.vertical { self.grid.add(elem, i, 0, 1, 1, settings) } else { self.grid.add(elem, 0, i, 1, 1, settings) }
    }
    pub fn add_default(&mut self, elem: Elem) {
        let s = self.grid.defaults;
        self.add(elem, s);
    }
    pub fn into_elem(self) -> Elem {
        Elem::Layout(Box::new(self.grid))
    }
}

/// `GridLayout.RowHelper`.
pub struct RowHelper {
    pub columns: usize,
    index: usize,
}

impl RowHelper {
    pub fn new(columns: usize) -> RowHelper {
        RowHelper { columns, index: 0 }
    }
    pub fn add(&mut self, grid: &mut Layout, elem: Elem, width: usize, settings: Settings) {
        let mut row = self.index / self.columns;
        let mut col = self.index % self.columns;
        if col + width > self.columns {
            row += 1;
            col = 0;
            self.index = self.index.div_ceil(self.columns) * self.columns;
        }
        self.index += width;
        grid.add(elem, row, col, 1, width, settings);
    }
}

/// `FrameLayout.alignInRectangle`.
pub fn align_in_rect(e: &mut Elem, ws: &mut Arena, x: i32, y: i32, w: i32, h: i32, ax: f32, ay: f32) {
    let ox = lerp(ax, 0.0, (w - e.w(ws)) as f32) as i32;
    e.set_x(ws, x + ox);
    let oy = lerp(ay, 0.0, (h - e.h(ws)) as f32) as i32;
    e.set_y(ws, y + oy);
}
