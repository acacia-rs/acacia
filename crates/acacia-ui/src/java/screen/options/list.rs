//! `OptionsList`: a `ContainerObjectSelectionList` 310 wide of rows of one or two option widgets
//! and of section headers.

use crate::java::text::Text;
use crate::java::widget::list::{Entry, SelectionList};
use crate::java::widget::text::{Fill, FocusableTextWidget};
use crate::java::widget::{Ctx, Widget, render};

/// `OptionsList.Entry`: its widgets placed at the screen's centre - 155, 160 apart, every frame.
struct Widgets {
    widgets: Vec<Box<dyn Widget>>,
    screen_width: i32,
}

impl Entry for Widgets {
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], _hovered: bool) {
        let x = self.screen_width / 2 - 155;
        for (i, w) in self.widgets.iter_mut().enumerate() {
            w.set_x(x + 160 * i as i32);
            w.set_y(row[1] + 2);
            render(w.as_mut(), ctx);
        }
    }
}

/// `OptionsList.HeaderEntry`: a borderless `FocusableTextWidget` centred under its padding.
struct Header {
    widget: FocusableTextWidget,
    pad_top: i32,
    screen_width: i32,
}

impl Entry for Header {
    fn extract_content(&mut self, ctx: &mut Ctx, row: [i32; 4], _hovered: bool) {
        let w = self.widget.width();
        self.widget.set_x(self.screen_width / 2 - w / 2);
        self.widget.set_y(row[1] + 2 + self.pad_top);
        render(&mut self.widget, ctx);
    }
}

pub struct OptionsList {
    pub list: SelectionList,
    screen_width: i32,
}

impl OptionsList {
    /// `new OptionsList(minecraft, width, screen)`: as high as the layout's contents, under its header.
    pub fn new(width: i32, height: i32, y: i32, in_world: bool) -> OptionsList {
        let mut list = SelectionList::new(width, height, y, 310, in_world);
        list.selectable = false;
        OptionsList { list, screen_width: width }
    }

    /// `addBig`: one widget the row's width.
    pub fn add_big(&mut self, mut widget: Box<dyn Widget>) {
        widget.base_mut().w = 310;
        self.list.push(Box::new(Widgets { widgets: vec![widget], screen_width: self.screen_width }), 25);
    }

    /// `addSmall`: two to a row.
    pub fn add_small(&mut self, widgets: Vec<Box<dyn Widget>>) {
        let mut it = widgets.into_iter().peekable();
        while let Some(first) = it.next() {
            let mut row = vec![first];
            row.extend(it.next());
            self.list.push(Box::new(Widgets { widgets: row, screen_width: self.screen_width }), 25);
        }
    }

    /// `addHeader`: 18 above it unless it is the first row.
    pub fn add_header(&mut self, ctx: &Ctx, text: Text) {
        let pad_top = if self.list.rows.is_empty() { 0 } else { 18 };
        let widget = FocusableTextWidget::new(ctx.font, text, 4, -1, false, Fill::OnFocus);
        self.list.push(Box::new(Header { widget, pad_top, screen_width: self.screen_width }), pad_top + 18);
    }

    /// `updateSizeAndPosition(width, height, 0, y)`: the scroll amount clamped to the new size.
    pub fn update_size(&mut self, width: i32, height: i32, y: i32) {
        let b = &mut self.list.base;
        (b.x, b.y, b.w, b.h) = (0, y, width, height);
        self.list.set_scroll(self.list.scroll);
    }
}
