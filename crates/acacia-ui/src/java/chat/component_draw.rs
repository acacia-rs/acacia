//! `ChatComponent.extractRenderState`: the line backgrounds, then each line's text and tag. In
//! the background (the HUD) that is all; in the foreground with the mouse tracked
//! (`DrawingFocusedGraphicsAccess`, the chat screen) the style under the mouse is found, a hovered
//! tag or icon sets its tooltip, and a message's icon shows while any of its lines is hovered. The
//! delayed-message queue line and the restricted prompt are not drawn.

use super::component::{Chat, Line, Tag, time_alpha};
use crate::java::active::{mouse_over, styles_under};
use crate::java::gui::{Gui, IDENTITY, Metrics, Pose, PoseOp, pose_apply};
use crate::java::mth::{as8, ceil, floor, white, with_alpha};
use crate::java::split::split;
use crate::java::text::{Lang, Style, Text};
use crate::java::tooltip::{self, Deferred, Positioner, Tooltip};

/// `DrawingFocusedGraphicsAccess`'s inputs and what it leaves for `extractDeferredElements`: the
/// last hovered style with a hover event (`hoveredTextStyle`) and the tooltip set this frame.
pub struct Focus<'a> {
    pub m: &'a dyn Metrics,
    pub lang: Lang<'a>,
    pub mouse: (i32, i32),
    pub hovered: Option<Style>,
    pub tooltip: Deferred,
}

impl<'a> Focus<'a> {
    pub fn new(m: &'a dyn Metrics, lang: Lang<'a>, mouse: (i32, i32)) -> Focus<'a> {
        Focus { m, lang, mouse, hovered: None, tooltip: Deferred::default() }
    }

    /// `handleMessage`'s style scan: whether any glyph of `text` is under the mouse.
    fn scan(&mut self, text: &Text, pose: &Pose, y: i32, color: u32) -> bool {
        let areas = self.m.active_areas(text, 0.0, y as f32, color, true);
        let hits = styles_under(&areas, pose, self.mouse);
        if let Some(style) = hits.iter().rev().find(|s| s.hover.is_some()) {
            self.hovered = Some((*style).clone());
        }
        !hits.is_empty()
    }

    /// `showTooltip`: the tag's text split to 210.
    fn tag_tooltip(&mut self, tag: &Tag) {
        let Some(text) = tag.text(self.lang) else { return };
        let lines = split(self.m, &text, 210).into_iter().map(tooltip::Line::Text).collect();
        self.tooltip.set(Tooltip { lines, x: self.mouse.0, y: self.mouse.1, positioner: Positioner::Default, style: None, extra_space: false }, false);
    }
}

fn rect(x0: i32, y0: i32, x1: i32, y1: i32) -> [f32; 4] {
    [x0 as f32, y0 as f32, x1 as f32, y1 as f32]
}

impl Chat {
    /// The HUD's chat (`BACKGROUND`), or the open chat screen's (`FOREGROUND`) when `focused`,
    /// at tick `ticks`, without the mouse.
    pub fn extract(&self, gui: &mut Gui, ticks: i32, focused: bool) {
        self.draw(gui, ticks, focused, None);
    }

    /// The chat screen's chat, hovered by `focus.mouse`.
    pub fn extract_focused(&self, gui: &mut Gui, ticks: i32, focus: &mut Focus) {
        self.draw(gui, ticks, true, Some(focus));
    }

    fn draw(&self, gui: &mut Gui, ticks: i32, focused: bool, mut focus: Option<&mut Focus>) {
        let total = self.lines.len() as i32;
        if total == 0 {
            return;
        }
        let o = &self.options;
        let scale = o.scale as f32;
        let max_width = ceil(o.width_px() as f32 / scale);
        let chat_bottom = floor((gui.height - 40) as f32 / scale);
        let text_opacity = o.opacity as f32 * 0.9 + 0.1;
        let background_opacity = o.background_opacity as f32;
        let entry_height = o.line_height();
        let bottom_to_text = (8.0 * (o.line_spacing + 1.0) - 4.0 * o.line_spacing).round() as i32;
        // forEachLine: oldest visible first, the transparent ones skipped.
        let per_page = self.lines_per_page(focused);
        let visible: Vec<(i32, &Line, f32)> = (0..(total - self.scroll).min(per_page))
            .rev()
            .map(|i| (i, &self.lines[(i + self.scroll) as usize]))
            .map(|(i, line)| (i, line, if focused { 1.0 } else { time_alpha(ticks, line.added) }))
            .filter(|l| l.2 > 1.0e-5)
            .collect();
        let ops = vec![PoseOp::Scale(scale, scale), PoseOp::Translate(4.0, 0.0)];
        let mut pose = IDENTITY;
        ops.iter().for_each(|&op| pose_apply(&mut pose, op));
        gui.push(ops);
        for &(i, _, alpha) in &visible {
            let bottom = chat_bottom - i * entry_height;
            gui.fill(-4, bottom - entry_height, max_width + 4 + 4, bottom, as8(alpha * background_opacity) << 24);
        }
        let mut hovered_message = false;
        for &(i, line, alpha) in &visible {
            let bottom = chat_bottom - i * entry_height;
            let (top, text_top) = (bottom - entry_height, bottom - bottom_to_text);
            let opacity = alpha * text_opacity;
            gui.text(line.content.clone(), 0, text_top, white(opacity), true);
            let hit = focus.as_deref_mut().is_some_and(|f| f.scan(&line.content, &pose, text_top, white(opacity)));
            hovered_message |= hit;
            let force_icon = line.end && std::mem::take(&mut hovered_message);
            let Some(tag) = &line.tag else { continue };
            gui.fill(-4, top, -2, bottom, with_alpha(as8(opacity), tag.indicator));
            let Some(f) = focus.as_deref_mut() else { continue };
            if mouse_over(&pose, f.mouse, rect(-4, top, -2, bottom)) {
                f.tag_tooltip(tag);
            }
            if let Some(icon) = tag.icon {
                let left = f.m.visual_width(&line.content) + 4;
                let (icon_top, icon_bottom) = (text_top + 9 - icon.height - 1, text_top + 9);
                let over = mouse_over(&pose, f.mouse, rect(left, icon_top, left + icon.width, icon_bottom));
                if over {
                    f.tag_tooltip(tag);
                }
                if force_icon || over {
                    gui.sprite(icon.sprite, left, icon_top, icon.width, icon.height);
                }
            }
        }
        if focused {
            let chat_height = visible.len() as i32 * entry_height;
            let virtual_height = total * entry_height;
            let y = self.scroll * chat_height / total - chat_bottom;
            let height = chat_height * chat_height / virtual_height;
            if virtual_height != chat_height {
                let alpha = if y > 0 { 170 } else { 96 };
                let color = if self.new_since_scroll { 0xCC3333 } else { 0x3333AA };
                let x = max_width + 4;
                gui.fill(x, -y, x + 2, -y - height, with_alpha(alpha, color));
                gui.fill(x + 2, -y, x + 1, -y - height, with_alpha(alpha, 0xCCCCCC));
            }
        }
        gui.pop();
    }
}
