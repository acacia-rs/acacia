//! `ClientTooltipPositioner`s: where a tooltip of a size goes for a mouse position or widget.

/// A positioner; widget rectangles are `[x, y, w, h]` (`ScreenRectangle`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Positioner {
    /// `DefaultTooltipPositioner`: right of and above the mouse, kept on screen.
    Default,
    /// `BelowOrAboveWidgetTooltipPositioner`: below the widget, else above it.
    BelowOrAbove([i32; 4]),
    /// `MenuTooltipPositioner`: beside the mouse, nudged off the widget.
    Menu([i32; 4]),
}

/// `MenuTooltipPositioner.getOffset`.
fn menu_offset(mouse_y: i32, widget_y: i32, widget_height: i32) -> i32 {
    let distance = (mouse_y - widget_y).abs().min(widget_height);
    let delta = distance as f32 / widget_height as f32;
    let start = (widget_height - 3) as f32;
    (start + delta * (5.0 - start) + 0.5).floor() as i32
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
            Positioner::BelowOrAbove([rx, ry, rw, rh]) => {
                let (mut x, mut y) = (rx + 3, ry + rh + 3 + 1);
                if y + h + 3 > screen[1] {
                    y = ry - h - 3 - 1;
                }
                if x + w > screen[0] {
                    x = (rx + rw - w - 3).max(4);
                }
                [x, y]
            }
            Positioner::Menu([_, ry, _, rh]) => {
                let mut rx = x + 12;
                if rx + w > screen[0] - 5 {
                    rx = (x - 12 - w).max(9);
                }
                let mut y = y + 3;
                let padded = h + 3 + 3;
                let lowest = ry + rh + 3 + menu_offset(0, 0, rh);
                if lowest + padded <= screen[1] - 5 {
                    y += menu_offset(y, ry, rh);
                } else {
                    y -= padded + menu_offset(y, ry + rh, rh);
                }
                [rx, y]
            }
        }
    }
}
