//! The recipe book's mouse handling: `RecipeBookComponent.mouseClicked` over its page
//! (`RecipeBookPage.mouseClicked`: the overlay, the arrows, the recipe buttons), the search box,
//! the filter and the tabs; `hasClickedOutside`; and the screen's toggle button.

use super::page::{self, PER_PAGE, overlay_buttons};
use super::screens::RecipeScreen;
use super::widgets::hovered;
use super::{Book, IMAGE_HEIGHT, IMAGE_WIDTH};
use crate::java::container::input::Button;
use crate::java::mth::floor_d;

/// What a press on the book did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookPress {
    /// A recipe to place (`tryPlaceRecipe`): an index into `Book::collections` and its entry.
    Recipe { collection: usize, entry: usize },
    /// Taken: a page turned, a tab or the filter switched, the search box focused, the overlay
    /// opened or closed.
    Changed,
}

/// What a press on a recipe book screen's own widgets did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipePress {
    Book(BookPress),
    /// The book's button: the book shown or hidden, the window moved.
    Toggled,
    /// Swallowed by the open book over a narrow screen.
    Swallowed,
}

fn at(mouse: [f64; 2]) -> [i32; 2] {
    [floor_d(mouse[0]), floor_d(mouse[1])]
}

impl Book {
    /// The visible tabs (`updateTabs`): index into `tabs` and the button's rectangle.
    pub fn tab_rects(&self) -> Vec<(usize, [i32; 4])> {
        let x = (self.width - IMAGE_WIDTH) / 2 - if self.narrow { 0 } else { 86 } - 30;
        let y = (self.height - IMAGE_HEIGHT) / 2 + 3;
        let visible = (0..self.tabs.len()).filter(|&i| self.tabs[i].search || self.tabs[i].collections.iter().any(|&c| self.collections[c].has_any_selected()));
        visible.enumerate().map(|(n, i)| (i, [x, y + 27 * n as i32, 35, 27])).collect()
    }

    /// `mouseClicked`.
    pub fn mouse_clicked(&mut self, mouse: [f64; 2], button: Button) -> Option<BookPress> {
        if !self.visible {
            return None;
        }
        if let Some(press) = self.page_clicked(mouse, button) {
            if matches!(press, BookPress::Recipe { .. }) && self.narrow {
                self.visible = false;
            }
            return Some(press);
        }
        let ([xo, yo], m) = (self.origin(), at(mouse));
        let left = button == Button::Left;
        if hovered([xo + 8, yo + 13, 17, 14], m) || left && hovered([xo + 25, yo + 13, 81, 14], m) {
            self.search.focused = true;
            return Some(BookPress::Changed);
        }
        self.search.focused = false;
        if left && hovered([xo + 110, yo + 12, 26, 16], m) {
            self.filtering = !self.filtering;
            if page::layout(self).2 <= self.page {
                self.page = 0;
            }
            return Some(BookPress::Changed);
        }
        let tab = self.tab_rects().into_iter().find(|&(_, rect)| left && hovered(rect, m));
        let (i, _) = tab?;
        if i != self.selected_tab {
            self.selected_tab = i;
            self.page = 0;
        }
        Some(BookPress::Changed)
    }

    /// `RecipeBookPage.mouseClicked`.
    fn page_clicked(&mut self, mouse: [f64; 2], button: Button) -> Option<BookPress> {
        let ([xo, yo], m) = (self.origin(), at(mouse));
        let (listed, page, total) = page::layout(self);
        if let Some(b) = self.overlay {
            let clicked = overlay_buttons(self, b).and_then(|(_, _, buttons)| buttons.into_iter().find(|o| hovered([o.at[0], o.at[1], 24, 24], m)));
            if let Some(o) = clicked.filter(|_| button == Button::Left) {
                return Some(BookPress::Recipe { collection: listed[page * PER_PAGE + b], entry: o.entry });
            }
            self.overlay = None;
            return Some(BookPress::Changed);
        }
        let left = button == Button::Left;
        if left && total > 1 && page + 1 < total && hovered([xo + 93, yo + 137, 12, 17], m) {
            self.page = page + 1;
            return Some(BookPress::Changed);
        }
        if left && total > 1 && page > 0 && hovered([xo + 38, yo + 137, 12, 17], m) {
            self.page = page - 1;
            return Some(BookPress::Changed);
        }
        let shown = listed.len().saturating_sub(page * PER_PAGE).min(PER_PAGE);
        let i = (0..shown).find(|&i| hovered([xo + 11 + 25 * (i as i32 % 5), yo + 31 + 25 * (i as i32 / 5), 25, 25], m))?;
        if button == Button::Middle {
            return None;
        }
        let collection = listed[page * PER_PAGE + i];
        let fitting: Vec<usize> = (0..self.collections[collection].entries.len())
            .filter(|&e| {
                let entry = &self.collections[collection].entries[e];
                if self.filtering { entry.craftable } else { entry.selected }
            })
            .collect();
        if left && !fitting.is_empty() {
            return Some(BookPress::Recipe { collection, entry: fitting[self.index() % fitting.len()] });
        }
        if !left && fitting.len() != 1 {
            self.overlay = Some(i);
        }
        Some(BookPress::Changed)
    }

    /// `hasClickedOutside` for the window at `(left, top)`: off it, off the book beside it and off
    /// the selected tab.
    pub fn clicked_outside(&self, mouse: [f64; 2], left: i32, top: i32, w: i32, h: i32) -> bool {
        if !self.visible {
            return true;
        }
        let [x, y] = mouse;
        let (l, t) = (f64::from(left), f64::from(top));
        let outside = x < l || y < t || x >= l + f64::from(w) || y >= t + f64::from(h);
        let on_book = l - 147.0 < x && x < l && t < y && y < t + f64::from(h);
        let tab = self.tab_rects().into_iter().find(|&(i, _)| i == self.selected_tab);
        outside && !on_book && !tab.is_some_and(|(_, rect)| hovered(rect, at(mouse)))
    }
}

impl RecipeScreen {
    /// `AbstractRecipeBookScreen.mouseClicked` before the slots: the book, then the book's button.
    pub fn mouse_clicked(&mut self, mouse: [f64; 2], button: Button) -> Option<RecipePress> {
        if let Some(press) = self.book.mouse_clicked(mouse, button) {
            return Some(RecipePress::Book(press));
        }
        if self.book.narrow && self.book.visible {
            return Some(RecipePress::Swallowed);
        }
        if button == Button::Left && hovered(self.button(), at(mouse)) {
            self.book.visible = !self.book.visible;
            self.base.left = self.book.screen_left(self.base.image_width);
            return Some(RecipePress::Toggled);
        }
        None
    }
}
