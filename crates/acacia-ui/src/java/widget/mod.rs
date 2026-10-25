//! Java 26.3's widgets (`net.minecraft.client.gui.components`) and layouts as the calls they make:
//! each widget keeps the state its Java class keeps and [`Widget::extract`] is its
//! `extractWidgetRenderState`. [`Ctx`] is what they draw through: the [`Gui`], the font, the mouse
//! and the scissor stack hovering is tested against.

pub mod button;
pub mod edit_box;
pub mod header_footer;
pub mod input;
pub mod layout;
pub mod list;
pub mod measure;
pub mod multiline;
pub mod multiline_box;
pub mod slider;
pub mod tabs;
pub mod text;
pub mod text_area;

use std::borrow::Cow;

use crate::java::gui::{Blend, Gui, GuiDraw, IDENTITY, Metrics, Pose, PoseOp, pose_apply, transform};
use crate::java::mth::{as8, floor, white};
use crate::java::text::{Lang, Style, Text};
use input::{Action, Key, KeyOutcome};

/// What a widget draws through for one frame.
pub struct Ctx<'a> {
    pub gui: &'a mut Gui,
    pub font: &'a dyn Metrics,
    pub lang: Lang<'a>,
    pub mouse: (i32, i32),
    /// `Util.getMillis()` now, for the caret blink.
    pub millis: i64,
    poses: Vec<Pose>,
    scissors: Vec<[i32; 4]>,
}

impl<'a> Ctx<'a> {
    pub fn new(gui: &'a mut Gui, font: &'a dyn Metrics, lang: Lang<'a>, mouse: (i32, i32)) -> Ctx<'a> {
        Ctx { gui, font, lang, mouse, millis: 0, poses: vec![IDENTITY], scissors: Vec::new() }
    }

    pub fn tr(&self, key: &str) -> Text {
        Text::translate(key, &[], self.lang)
    }

    pub fn tr_args(&self, key: &str, args: &[Text]) -> Text {
        Text::translate(key, args, self.lang)
    }

    pub fn width(&self, t: &Text) -> i32 {
        self.font.width(t)
    }

    pub fn push(&mut self, ops: Vec<PoseOp>) {
        let mut m = *self.poses.last().unwrap_or(&IDENTITY);
        for op in &ops {
            pose_apply(&mut m, *op);
        }
        self.poses.push(m);
        self.gui.push(ops);
    }

    pub fn pop(&mut self) {
        if self.poses.len() > 1 {
            self.poses.pop();
        }
        self.gui.pop();
    }

    /// `enableScissor`: the box through the pose, cut by the enclosing scissor.
    pub fn enable_scissor(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.gui.scissor(x0, y0, x1, y1);
        let m = self.poses.last().unwrap_or(&IDENTITY);
        let (a, b) = (transform(m, x0 as f32, y0 as f32), transform(m, x1 as f32, y1 as f32));
        let r = [floor(a[0].min(b[0])), floor(a[1].min(b[1])), floor(a[0].max(b[0])), floor(a[1].max(b[1]))];
        let last = self.scissors.last().copied().unwrap_or([0, 0, self.gui.width, self.gui.height]);
        let cut = [r[0].max(last[0]), r[1].max(last[1]), r[2].min(last[2]), r[3].min(last[3])];
        self.scissors.push(if cut[0] < cut[2] && cut[1] < cut[3] { cut } else { [0, 0, 0, 0] });
    }

    pub fn disable_scissor(&mut self) {
        self.scissors.pop();
        self.gui.unscissor();
    }

    /// `containsPointInScissor`: Java's `ScissorStack.containsPoint` tests the first scissor
    /// pushed, not the innermost one.
    pub fn in_scissor(&self, x: i32, y: i32) -> bool {
        self.scissors.first().is_none_or(|r| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
    }

    /// `isHovered` as `AbstractWidget.extractRenderState` sets it.
    pub fn hovered(&self, b: &Base) -> bool {
        self.in_scissor(self.mouse.0, self.mouse.1) && b.contains(self.mouse.0, self.mouse.1)
    }

    /// A text collector's `accept(LEFT, x, y, text)` at `opacity`.
    pub fn label(&mut self, text: Text, x: i32, y: i32, opacity: f32) {
        if as8(opacity) != 0 {
            self.gui.text(text, x, y, white(opacity), true);
        }
    }

    /// A text collector's `accept(CENTER, anchor, y, text)`.
    pub fn label_centered(&mut self, text: Text, anchor: i32, y: i32, opacity: f32) {
        let w = self.width(&text);
        self.label(text, anchor - w / 2, y, opacity);
    }

    /// `AbstractWidget.extractScrollingStringOverContents(output, message, margin)`.
    pub fn scrolling_over(&mut self, b: &Base, message: Text, margin: i32, opacity: f32) {
        let (left, right) = (b.x + margin, b.x + b.w - margin);
        self.gui.scrolling(message, (left + right) / 2, left, right, b.y, b.y + b.h, white(opacity));
    }

    /// `blit(GUI_TEXTURED, texture, x, y, u, v, w, h, texW, texH)`; `texture` without `textures/`
    /// and `.png`.
    pub fn blit(&mut self, texture: &'static str, x: i32, y: i32, uv: [f32; 2], w: i32, h: i32, tex: [i32; 2]) {
        self.gui.draws.push(GuiDraw::Blit { texture: texture.into(), blend: Blend::Textured, x, y, w, h, uv, src: [w, h], tex, color: 0xFFFF_FFFF });
    }

    /// `GuiGraphicsExtractor.centeredText`: centred on `x` with a shadow.
    pub fn centered_text(&mut self, text: Text, x: i32, y: i32, color: u32) {
        let w = self.width(&text);
        self.gui.text(text, x - w / 2, y, color, true);
    }
}

/// `AbstractWidget`'s own state.
#[derive(Clone, Debug)]
pub struct Base {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub message: Text,
    pub active: bool,
    pub visible: bool,
    pub alpha: f32,
    pub focused: bool,
    /// What the client knows the widget by (its `onPress`): a translation key or an option id.
    pub id: Cow<'static, str>,
}

impl Base {
    pub fn new(x: i32, y: i32, w: i32, h: i32, message: Text) -> Base {
        Base { x, y, w, h, message, active: true, visible: true, alpha: 1.0, focused: false, id: Cow::Borrowed("") }
    }

    pub fn with_id(mut self, id: impl Into<Cow<'static, str>>) -> Base {
        self.id = id.into();
        self
    }

    /// `areCoordinatesInRectangle`.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn is_active(&self) -> bool {
        self.visible && self.active
    }

    /// `WithInactiveMessage.getMessage`: greyed (`-6250336` merged under the message's style) when
    /// inactive.
    pub fn shown_message(&self) -> Text {
        if self.active { self.message.clone() } else { inactive(&self.message) }
    }
}

/// `WithInactiveMessage.defaultInactiveMessage`.
pub fn inactive(message: &Text) -> Text {
    message.clone().with_style(&Style::color(INACTIVE & 0xFF_FFFF))
}

/// `-6250336`, the inactive label colour (a style colour drops the alpha).
pub const INACTIVE: u32 = 0xFFA0_A0A0;

/// A widget: its `AbstractWidget` state and its `extractWidgetRenderState`.
pub trait Widget {
    fn base(&self) -> &Base;
    fn base_mut(&mut self) -> &mut Base;
    /// `extractWidgetRenderState`; `hovered` is `isHovered` for this frame.
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool);
    /// `getWidth`, for widgets that compute it.
    fn width(&self) -> i32 {
        self.base().w
    }
    fn height(&self) -> i32 {
        self.base().h
    }
    fn set_x(&mut self, x: i32) {
        self.base_mut().x = x;
    }
    fn set_y(&mut self, y: i32) {
        self.base_mut().y = y;
    }
    /// `onClick`, once `mouseClicked` found it active and under the mouse.
    fn on_click(&mut self, _x: f64, _shift: bool) -> Option<Action> {
        None
    }
    /// `onDrag` while the mouse is held after clicking it.
    fn on_drag(&mut self, _x: f64) -> Option<Action> {
        None
    }
    /// `onRelease`.
    fn on_release(&mut self) {}
    /// `keyPressed` while focused.
    fn key_pressed(&mut self, _key: Key, _shift: bool) -> KeyOutcome {
        None
    }
    /// `setFocused`; `pointer` when the last input was the mouse or Tab.
    fn set_focused(&mut self, focused: bool, _pointer: bool) {
        self.base_mut().focused = focused;
    }
    /// A slider's `canChangeValue`.
    fn can_change(&self) -> bool {
        false
    }
    fn set_can_change(&mut self, _on: bool) {}
}

/// `AbstractWidget.extractRenderState`: nothing when invisible.
pub fn render(w: &mut dyn Widget, ctx: &mut Ctx) {
    if w.base().visible {
        let b = w.base().clone();
        let hovered = ctx.in_scissor(ctx.mouse.0, ctx.mouse.1) && Base { w: w.width(), h: w.height(), ..b }.contains(ctx.mouse.0, ctx.mouse.1);
        w.extract(ctx, hovered);
    }
}
