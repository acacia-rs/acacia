//! Tooltips as `GuiGraphicsExtractor.tooltip` draws them: `ClientTooltipComponent` lines measured
//! and placed by a `ClientTooltipPositioner`, `TooltipRenderUtil`'s nine-sliced background and
//! frame, then each line's text and image. `item` builds an item's lines
//! (`ItemStack.getTooltipLines`); `stack` is the item data they read.

pub mod item;
pub mod stack;

use std::borrow::Cow;

use super::gui::{Gui, Metrics};
use super::mth::WHITE;
use super::picture::Picture;
use super::text::Text;

/// A `ClientTooltipComponent`.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// `ClientTextTooltip`: 10 high.
    Text(Text),
    /// An image component (`ClientBundleTooltip`) by its Java class name and size.
    Image { kind: Cow<'static, str>, w: i32, h: i32 },
}

impl Line {
    pub fn width(&self, font: &dyn Metrics) -> i32 {
        match self {
            Line::Text(t) => font.width(t),
            Line::Image { w, .. } => *w,
        }
    }

    pub fn height(&self) -> i32 {
        match self {
            Line::Text(_) => 10,
            Line::Image { h, .. } => *h,
        }
    }
}

/// A `ClientTooltipPositioner`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Positioner {
    /// `DefaultTooltipPositioner`: right of and above the mouse, kept on screen.
    Default,
    /// `MenuTooltipPositioner` of a widget at `[x, y, w, h]`: beside the mouse, clear of the widget.
    Menu([i32; 4]),
}

impl Positioner {
    /// `positionTooltip(screenWidth, screenHeight, x, y, tooltipWidth, tooltipHeight)`.
    pub fn place(self, screen: [i32; 2], x: i32, y: i32, w: i32, h: i32) -> [i32; 2] {
        match self {
            Positioner::Default => {
                let (mut x, mut y) = (x + 12, y - 12);
                if x + w > screen[0] {
                    x = (x - 24 - w).max(4);
                }
                let padded = h + 3;
                if y + padded > screen[1] {
                    y = screen[1] - padded;
                }
                [x, y]
            }
            Positioner::Menu([_, top, _, rh]) => {
                let offset = |mouse: i32, widget: i32| {
                    let distance = (mouse - widget).abs().min(rh);
                    let v = super::mth::lerp(distance as f32 / rh as f32, (rh - 3) as f32, 5.0);
                    (v + 0.5).floor() as i32
                };
                let mut out = [x + 12, y + 3];
                if out[0] + w > screen[0] - 5 {
                    out[0] = (x - 12 - w).max(9);
                }
                let padded = h + 6;
                let bottom = top + rh;
                if bottom + 3 + offset(0, 0) + padded <= screen[1] - 5 {
                    out[1] += offset(out[1], top);
                } else {
                    out[1] -= padded + offset(out[1], bottom);
                }
                out
            }
        }
    }
}

/// A tooltip to draw: what `setTooltipForNextFrame` defers.
#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    pub lines: Vec<Line>,
    pub x: i32,
    pub y: i32,
    pub positioner: Positioner,
    /// The `tooltip_style` id (`minecraft:fancy`), `None` for the default sprites.
    pub style: Option<String>,
    /// Two pixels after the first line (an item's name).
    pub extra_space: bool,
}

/// `GuiGraphicsExtractor.deferredTooltip`: the first tooltip set in a frame wins unless a later one
/// replaces it; drawn by `extractDeferredElements` after everything else.
#[derive(Clone, Debug, Default)]
pub struct Deferred(pub Option<Tooltip>);

impl Deferred {
    /// `setTooltipForNextFrameInternal`: nothing for no lines.
    pub fn set(&mut self, tooltip: Tooltip, replace: bool) {
        if !tooltip.lines.is_empty() && (self.0.is_none() || replace) {
            self.0 = Some(tooltip);
        }
    }

    /// `extractDeferredElements`' tooltip part: a new stratum, then the tooltip.
    pub fn extract(&mut self, gui: &mut Gui, font: &dyn Metrics) {
        if let Some(t) = self.0.take() {
            gui.next_stratum();
            extract(gui, font, &t);
        }
    }
}

/// `TooltipRenderUtil.extractTooltipBackground`: background and frame around the `w` × `h` text area.
pub fn background(gui: &mut Gui, x: i32, y: i32, w: i32, h: i32, style: Option<&str>) {
    let (x0, y0) = (x - 3 - 9, y - 3 - 9);
    let (pw, ph) = (w + 3 + 3 + 18, h + 3 + 3 + 18);
    let path = |part: &str| match style {
        None => format!("tooltip/{part}"),
        Some(id) => format!("tooltip/{}_{part}", id.split_once(':').map_or(id, |(_, p)| p)),
    };
    gui.sprite(path("background"), x0, y0, pw, ph);
    gui.sprite(path("frame"), x0, y0, pw, ph);
}

/// `GuiGraphicsExtractor.tooltip`.
pub fn extract(gui: &mut Gui, font: &dyn Metrics, t: &Tooltip) {
    let mut w = 0;
    let mut h = if t.extra_space && t.lines.len() != 1 { 0 } else { -2 };
    for line in &t.lines {
        w = w.max(line.width(font));
        h += line.height();
    }
    let [x, y] = t.positioner.place([gui.width, gui.height], t.x, t.y, w, h);
    gui.push(Vec::new());
    background(gui, x, y, w, h, t.style.as_deref());
    let gap = |i: usize| if t.extra_space && i == 0 { 2 } else { 0 };
    let mut ly = y;
    for (i, line) in t.lines.iter().enumerate() {
        if let Line::Text(text) = line {
            gui.text(text.clone(), x, ly, WHITE, true);
        }
        ly += line.height() + gap(i);
    }
    ly = y;
    for (i, line) in t.lines.iter().enumerate() {
        if let Line::Image { kind, w: iw, h: ih } = line {
            gui.draws.push(super::gui::GuiDraw::Picture(Picture::TooltipImage { kind: kind.to_string(), x, y: ly, w: *iw, h: *ih }));
        }
        ly += line.height() + gap(i);
    }
    gui.pop();
}
