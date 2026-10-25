//! `HeaderAndFooterLayout`: a header frame across the top, a footer frame across the bottom and
//! the contents 30 below the header, or as low as fits above the footer.

use super::layout::{Arena, Elem, Kind, Layout};

pub struct HeaderFooter {
    pub header: Layout,
    pub contents: Layout,
    pub footer: Layout,
    pub header_h: i32,
    pub footer_h: i32,
}

impl HeaderFooter {
    /// `new HeaderAndFooterLayout(screen, headerHeight, footerHeight)`.
    pub fn new(header_h: i32, footer_h: i32) -> HeaderFooter {
        HeaderFooter { header: Layout::frame(0, 0), contents: Layout::frame(0, 0), footer: Layout::frame(0, 0), header_h, footer_h }
    }

    /// `getContentHeight`.
    pub fn content_height(&self, height: i32) -> i32 {
        height - self.header_h - self.footer_h
    }

    pub fn add_header(&mut self, e: Elem) {
        let s = self.header.defaults;
        self.header.add_child(e, s);
    }

    pub fn add_contents(&mut self, e: Elem) {
        let s = self.contents.defaults;
        self.contents.add_child(e, s);
    }

    pub fn add_footer(&mut self, e: Elem) {
        let s = self.footer.defaults;
        self.footer.add_child(e, s);
    }

    /// `arrangeElements` on a `width` x `height` screen.
    pub fn arrange(&mut self, ws: &mut Arena, width: i32, height: i32) {
        self.header.kind = Kind::Frame { min_w: width, min_h: self.header_h };
        self.header.set_x(ws, 0);
        self.header.set_y(ws, 0);
        self.header.arrange(ws);
        self.footer.kind = Kind::Frame { min_w: width, min_h: self.footer_h };
        self.footer.arrange(ws);
        self.footer.set_y(ws, height - self.footer_h);
        self.contents.kind = Kind::Frame { min_w: width, min_h: 0 };
        self.contents.arrange(ws);
        let y = (self.header_h + 30).min(height - self.footer_h - self.contents.h);
        self.contents.set_x(ws, 0);
        self.contents.set_y(ws, y);
    }
}
