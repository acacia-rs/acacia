//! What Java's `GuiGraphicsExtractor` is asked to draw, as a list of calls: sprite blits, texture
//! blits, fills, text, items, pose pushes and scissors. The Java look's layouts write these
//! (`Gui`); `paint.rs` turns them into quads, and the parity probes compare them with the calls the
//! game makes for the same state. Ported from Horizon's `hud_layout.rs` (MIT).

use std::borrow::Cow;

use super::mth::{WHITE, ceil};
use super::text::{Style, Text};

/// A `Matrix3x2fStack` operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PoseOp {
    Translate(f32, f32),
    Scale(f32, f32),
}

/// A JOML `Matrix3x2f`: `[m00, m01, m10, m11, m20, m21]`.
pub type Pose = [f32; 6];
pub const IDENTITY: Pose = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `op` applied as JOML's `translate` / `scale` do, in the same float order.
pub fn pose_apply(m: &mut Pose, op: PoseOp) {
    match op {
        PoseOp::Translate(x, y) => {
            m[4] += m[0] * x + m[2] * y;
            m[5] += m[1] * x + m[3] * y;
        }
        PoseOp::Scale(x, y) => {
            m[0] *= x;
            m[1] *= x;
            m[2] *= y;
            m[3] *= y;
        }
    }
}

/// `(x, y)` through `m`.
pub fn transform(m: &Pose, x: f32, y: f32) -> [f32; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

/// A blit's render pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// `GUI_TEXTURED`.
    Textured,
    /// `VIGNETTE`: `dst * (1 - src)`.
    Vignette,
    /// `GUI_NAUSEA_OVERLAY`: additive.
    Nausea,
    /// `CROSSHAIR`: the colour under it inverted.
    Crosshair,
}

impl Blend {
    pub fn pipeline(self) -> &'static str {
        match self {
            Blend::Textured => "pipeline/gui_textured",
            Blend::Vignette => "pipeline/vignette",
            Blend::Nausea => "pipeline/gui_nausea_overlay",
            Blend::Crosshair => "pipeline/crosshair",
        }
    }
}

/// One `GuiGraphicsExtractor` call. Colours are ARGB.
#[derive(Clone, Debug, PartialEq)]
pub enum GuiDraw {
    /// `blitSprite(pipeline, sprite, x, y, w, h, color)`; `region` is `(spriteW, spriteH, u, v)`
    /// for the overload drawing part of a sprite.
    Sprite { sprite: Cow<'static, str>, x: i32, y: i32, w: i32, h: i32, region: Option<[i32; 4]>, color: u32, blend: Blend },
    /// `blitSprite` with a block atlas sprite (the nether portal overlay).
    AtlasSprite { sprite: Cow<'static, str>, x: i32, y: i32, w: i32, h: i32, color: u32 },
    /// `blit(pipeline, texture, x, y, u, v, w, h, srcW, srcH, texW, texH, color)`; `texture` is the
    /// path under `textures/` without `.png`.
    Blit { texture: Cow<'static, str>, blend: Blend, x: i32, y: i32, w: i32, h: i32, uv: [f32; 2], src: [i32; 2], tex: [i32; 2], color: u32 },
    /// `fill(x0, y0, x1, y1, color)`, corners sorted.
    Fill { x0: i32, y0: i32, x1: i32, y1: i32, color: u32 },
    /// `fillGradient(x0, y0, x1, y1, top, bottom)`.
    Gradient { x0: i32, y0: i32, x1: i32, y1: i32, top: u32, bottom: u32 },
    /// `text(font, text, x, y, color, shadow)`; legacy codes in `text` are resolved when drawn.
    Text { text: Text, x: i32, y: i32, color: u32, shadow: bool },
    /// `item(stack, x, y, seed)`: `key` names the stack to the caller (a slot number).
    Item { key: i32, x: i32, y: i32, seed: i32 },
    /// `itemDecorations(font, stack, x, y, count)`, drawn by the caller with the item.
    Decorations { key: i32, x: i32, y: i32, count: Option<String> },
    /// `pose().pushMatrix()` followed by `ops`.
    Push(Vec<PoseOp>),
    /// `pose().popMatrix()`.
    Pop,
    /// `enableScissor(x0, y0, x1, y1)`.
    Scissor { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// `disableScissor()`.
    Unscissor,
    /// `ActiveTextCollector.acceptScrolling(text, centerX, x0, x1, y0, y1)`: a label centred in
    /// the box, or scrolled to the clock when wider; `color` is `ARGB.white(opacity)`.
    Scrolling { text: Text, center_x: i32, x0: i32, x1: i32, y0: i32, y1: i32, color: u32 },
    /// `textHighlight(x0, y0, x1, y1, invert)`: a `GUI_INVERT` fill when `invert`, then a
    /// `GUI_TEXT_HIGHLIGHT` one.
    Highlight { x0: i32, y0: i32, x1: i32, y1: i32, invert: bool },
    /// `nextStratum()`: what follows draws over everything before, whatever its bounds.
    Stratum,
    /// `blurBeforeThisStratum()`: the strata before this one are blurred (a screen's background).
    Blur,
    /// A picture-in-picture or tooltip image, recorded by its parameters.
    Picture(super::picture::Picture),
}

/// A screen `width` × `height` GUI pixels big being drawn into.
#[derive(Debug, Clone, Default)]
pub struct Gui {
    pub draws: Vec<GuiDraw>,
    pub width: i32,
    pub height: i32,
}

impl Gui {
    pub fn new(width: i32, height: i32) -> Gui {
        Gui { draws: Vec::new(), width, height }
    }

    pub fn sprite(&mut self, sprite: impl Into<Cow<'static, str>>, x: i32, y: i32, w: i32, h: i32) {
        self.sprite_tinted(sprite, x, y, w, h, WHITE);
    }

    pub fn sprite_tinted(&mut self, sprite: impl Into<Cow<'static, str>>, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.draws.push(GuiDraw::Sprite { sprite: sprite.into(), x, y, w, h, region: None, color, blend: Blend::Textured });
    }

    /// `blitSprite(sprite, spriteW, spriteH, u, v, x, y, w, h)`: part of a `size` sprite from `uv`.
    pub fn region(&mut self, sprite: impl Into<Cow<'static, str>>, size: [i32; 2], uv: [i32; 2], x: i32, y: i32, w: i32, h: i32) {
        self.draws.push(GuiDraw::Sprite { sprite: sprite.into(), x, y, w, h, region: Some([size[0], size[1], uv[0], uv[1]]), color: WHITE, blend: Blend::Textured });
    }

    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        self.draws.push(GuiDraw::Fill { x0: x0.min(x1), y0: y0.min(y1), x1: x0.max(x1), y1: y0.max(y1), color });
    }

    pub fn gradient(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, top: u32, bottom: u32) {
        self.draws.push(GuiDraw::Gradient { x0, y0, x1, y1, top, bottom });
    }

    /// `text`; nothing when `color` is fully transparent, as the game skips it.
    pub fn text(&mut self, text: impl Into<Text>, x: i32, y: i32, color: u32, shadow: bool) {
        if color >> 24 != 0 {
            self.draws.push(GuiDraw::Text { text: text.into(), x, y, color, shadow });
        }
    }

    /// `drawCenteredString`: centred on `x` with a shadow.
    pub fn centered(&mut self, text: Text, width: i32, x: i32, y: i32, color: u32) {
        self.text(text, x - width / 2, y, color, true);
    }

    pub fn item(&mut self, key: i32, x: i32, y: i32, seed: i32) {
        self.draws.push(GuiDraw::Item { key, x, y, seed });
    }

    pub fn decorations(&mut self, key: i32, x: i32, y: i32, count: Option<String>) {
        self.draws.push(GuiDraw::Decorations { key, x, y, count });
    }

    pub fn push(&mut self, ops: Vec<PoseOp>) {
        self.draws.push(GuiDraw::Push(ops));
    }

    pub fn pop(&mut self) {
        self.draws.push(GuiDraw::Pop);
    }

    pub fn scissor(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.draws.push(GuiDraw::Scissor { x0, y0, x1, y1 });
    }

    pub fn unscissor(&mut self) {
        self.draws.push(GuiDraw::Unscissor);
    }

    pub fn scrolling(&mut self, text: Text, center_x: i32, x0: i32, x1: i32, y0: i32, y1: i32, color: u32) {
        self.draws.push(GuiDraw::Scrolling { text, center_x, x0, x1, y0, y1, color });
    }

    pub fn highlight(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, invert: bool) {
        self.draws.push(GuiDraw::Highlight { x0, y0, x1, y1, invert });
    }

    pub fn next_stratum(&mut self) {
        self.draws.push(GuiDraw::Stratum);
    }

    pub fn blur(&mut self) {
        self.draws.push(GuiDraw::Blur);
    }
}

/// Measures text in the font the calls are drawn with.
pub trait Metrics {
    /// `Font.getGlyphSource(style.font).getGlyph(c).info().getAdvance(style.isBold())`.
    fn advance(&self, c: char, style: &Style) -> f32;

    /// `Font.width` of styled text: the advances summed as floats, then `Mth.ceil`; legacy codes
    /// resolved first.
    fn width(&self, text: &Text) -> i32 {
        let text = text.resolve_legacy();
        ceil(text.chars().map(|(c, s)| self.advance(c, s)).sum())
    }
}

/// Every character as wide as the field, plus 1 for bold; for tests.
pub struct Fixed(pub f32);

impl Metrics for Fixed {
    fn advance(&self, _: char, style: &Style) -> f32 {
        self.0 + f32::from(u8::from(style.is_bold()))
    }
}
