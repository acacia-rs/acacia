//! `LecternScreen`, a `BookViewScreen` with the lectern's buttons: the book background, the page
//! indicator and up to 14 wrapped lines of the open page, page buttons and Done / Take Book.

use super::{blit, transparent_background};
use crate::java::gui::{Gui, Metrics};
use crate::java::text::{Lang, Style, Text};
use crate::java::tooltip::Deferred;
use crate::java::widget::button::Button;
use crate::java::widget::measure::split;
use crate::java::widget::{Base, Ctx, Widget, render};

const IMAGE: i32 = 192;
const TOP: i32 = 2;

/// `PAGE_TEXT_STYLE`: black, no shadow.
fn page_style() -> Style {
    Style { color: Some(0), shadow_color: Some(0), ..Style::default() }
}

/// `PageButton`: its arrow sprite, lit while hovered or focused.
struct PageButton {
    base: Base,
    forward: bool,
}

impl Widget for PageButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let lit = if hovered || self.base.focused { "_highlighted" } else { "" };
        let dir = if self.forward { "forward" } else { "backward" };
        ctx.gui.sprite(format!("widget/page_{dir}{lit}"), self.base.x, self.base.y, 23, 13);
    }
}

pub struct Lectern {
    pub width: i32,
    pub height: i32,
    /// `BookAccess.pages`.
    pub pages: Vec<Text>,
    pub page: i32,
    /// `player.mayBuild()`: Done and Take Book instead of a wide Done.
    pub may_build: bool,
}

impl Lectern {
    /// `extractRenderStateWithTooltipAndSubtitles`.
    pub fn frame(&self, gui: &mut Gui, font: &dyn Metrics, lang: Lang, mouse: [i32; 2]) {
        let left = (self.width - IMAGE) / 2;
        gui.next_stratum();
        transparent_background(gui);
        blit(gui, "gui/book", left, TOP, [0.0, 0.0], IMAGE, IMAGE);
        gui.next_stratum();
        let mut ctx = Ctx::new(gui, font, lang, (mouse[0], mouse[1]));
        let controls_top = TOP + IMAGE + 2;
        let mut buttons: Vec<Box<dyn Widget>> = if self.may_build {
            let middle = self.width / 2;
            vec![
                Box::new(Button::new(middle - 98 - 2, controls_top, 98, 20, ctx.tr("gui.done"))),
                Box::new(Button::new(middle + 2, controls_top, 98, 20, ctx.tr("lectern.take_book"))),
            ]
        } else {
            vec![Box::new(Button::new((self.width - 200) / 2, controls_top, 200, 20, ctx.tr("gui.done")))]
        };
        let count = self.pages.len() as i32;
        let mut forward = PageButton { base: Base::new(left + 116, TOP + 157, 23, 13, ctx.tr("book.page_button.next")), forward: true };
        let mut back = PageButton { base: Base::new(left + 43, TOP + 157, 23, 13, ctx.tr("book.page_button.previous")), forward: false };
        forward.base.visible = self.page < count - 1;
        back.base.visible = self.page > 0;
        buttons.push(Box::new(forward));
        buttons.push(Box::new(back));
        for b in &mut buttons {
            render(b.as_mut(), &mut ctx);
        }
        // visitText fills its cache only when currentPage differs from the initial cachedPage of -1,
        // which an empty book's clamp leaves it at: the indicator stays empty and no line shows.
        let (indicator, lines) = if self.page == -1 {
            (Text::default(), Vec::new())
        } else {
            let style = page_style();
            let args = [Text::literal((self.page + 1).to_string()), Text::literal(count.max(1).to_string())];
            let page = usize::try_from(self.page).ok().and_then(|p| self.pages.get(p)).cloned().unwrap_or_default();
            (Text::translate("book.pageIndicator", &args, lang).with_style(&style), split(font, &page.with_style(&style), 114))
        };
        let w = font.width(&indicator);
        ctx.gui.text(indicator, left + 148 - w, TOP + 16, 0xFFFF_FFFF, true);
        for (i, line) in lines.into_iter().take(128 / 9).enumerate() {
            ctx.gui.text(line, left + 36, TOP + 30 + i as i32 * 9, 0xFFFF_FFFF, true);
        }
        Deferred::default().extract(gui, font);
    }
}
