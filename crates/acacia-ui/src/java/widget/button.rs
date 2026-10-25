//! `AbstractButton` and its kinds: `Button`, `ImageButton`, `SpriteIconButton`, `CycleButton` and
//! `Checkbox`.

use std::borrow::Cow;

use super::text::MultiLineText;
use super::{Base, Ctx, Widget};
use crate::java::gui::Metrics;
use crate::java::mth::white;
use crate::java::text::Text;

/// `WidgetSprites`: (enabled, disabled, enabled focused, disabled focused).
#[derive(Clone, Debug, PartialEq)]
pub struct Sprites(pub Cow<'static, str>, pub Cow<'static, str>, pub Cow<'static, str>, pub Cow<'static, str>);

impl Sprites {
    pub fn one(s: impl Into<Cow<'static, str>>) -> Sprites {
        let s = s.into();
        Sprites(s.clone(), s.clone(), s.clone(), s)
    }

    /// `WidgetSprites(enabled, disabled, focused)`.
    pub fn three(enabled: &'static str, disabled: &'static str, focused: &'static str) -> Sprites {
        Sprites(enabled.into(), disabled.into(), focused.into(), disabled.into())
    }

    pub fn get(&self, enabled: bool, focused: bool) -> Cow<'static, str> {
        match (enabled, focused) {
            (true, false) => self.0.clone(),
            (false, false) => self.1.clone(),
            (true, true) => self.2.clone(),
            (false, true) => self.3.clone(),
        }
    }
}

pub fn button_sprites() -> Sprites {
    Sprites::three("widget/button", "widget/button_disabled", "widget/button_highlighted")
}

/// `AbstractButton.extractDefaultSprite`; `highlight` overrides hovered-or-focused when set.
pub fn default_sprite(ctx: &mut Ctx, b: &Base, hovered: bool, highlight: Option<bool>) {
    let lit = highlight.unwrap_or(hovered || b.focused);
    ctx.gui.sprite_tinted(button_sprites().get(b.active, lit), b.x, b.y, b.w, b.h, white(b.alpha));
}

/// `Button.Plain`: the button sprite and its label scrolling over it.
#[derive(Clone, Debug)]
pub struct Button {
    pub base: Base,
    /// `setOverrideRenderHighlightedSprite`.
    pub highlight: Option<bool>,
}

impl Button {
    pub fn new(x: i32, y: i32, w: i32, h: i32, message: Text) -> Button {
        Button { base: Base::new(x, y, w, h, message), highlight: None }
    }

    /// `Button.builder(message).width(w).build()`: 20 high at the origin.
    pub fn sized(w: i32, message: Text) -> Button {
        Button::new(0, 0, w, 20, message)
    }
}

impl Widget for Button {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        default_sprite(ctx, &self.base, hovered, self.highlight);
        ctx.scrolling_over(&self.base, self.base.shown_message(), 2, self.base.alpha);
    }
}

/// `ImageButton`: one sprite by state, no label.
#[derive(Clone, Debug)]
pub struct ImageButton {
    pub base: Base,
    pub sprites: Sprites,
}

impl Widget for ImageButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let b = &self.base;
        ctx.gui.sprite(self.sprites.get(b.is_active(), hovered || b.focused), b.x, b.y, b.w, b.h);
    }
}

/// `SpriteIconButton`: `CenteredIcon` (icon only) or `TextAndIcon` (label left, icon right).
#[derive(Clone, Debug)]
pub struct SpriteIconButton {
    pub base: Base,
    pub sprite: Sprites,
    pub sprite_size: [i32; 2],
    pub sprite_offset: [i32; 2],
    pub icon_only: bool,
    pub loading: bool,
}

impl SpriteIconButton {
    pub fn new(w: i32, h: i32, message: Text, sprite: Sprites, sprite_size: [i32; 2], icon_only: bool) -> SpriteIconButton {
        SpriteIconButton { base: Base::new(0, 0, w, h, message), sprite, sprite_size, sprite_offset: [0, 0], icon_only, loading: false }
    }

    fn icon(&self, ctx: &mut Ctx, hovered: bool, x: i32, y: i32) {
        let b = &self.base;
        let active = b.is_active() && !self.loading;
        let [w, h] = self.sprite_size;
        ctx.gui.sprite_tinted(self.sprite.get(active, hovered || b.focused), x, y, w, h, white(b.alpha));
    }
}

impl Widget for SpriteIconButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let b = self.base.clone();
        let [sw, sh] = self.sprite_size;
        if self.loading {
            ctx.gui.sprite_tinted("widget/button_disabled", b.x, b.y, b.w, b.h, white(b.alpha));
            ctx.gui.sprite_tinted("friends/loading", b.x + (b.w - 5) / 2, b.y + (b.h - 2) / 2, 5, 2, white(b.alpha));
            return;
        }
        default_sprite(ctx, &b, hovered, None);
        let y = self.sprite_offset[1] + b.y + b.h / 2 - sh / 2;
        if self.icon_only {
            self.icon(ctx, hovered, self.sprite_offset[0] + b.x + b.w / 2 - sw / 2, y);
        } else {
            let (left, right) = (b.x + 2, b.x + b.w - sw - 4);
            ctx.gui.scrolling(b.shown_message(), b.x + b.w / 2, left, right, b.y, b.y + b.h, white(b.alpha));
            self.icon(ctx, hovered, self.sprite_offset[0] + b.x + b.w - sw - 2, y);
        }
    }
}

/// `CycleButton.DisplayState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    NameAndValue,
    Value,
    Hide,
}

/// `CycleButton`: a button whose message names its value; `sprite` replaces the button sprite.
#[derive(Clone, Debug)]
pub struct CycleButton {
    pub base: Base,
    pub display: Display,
    pub sprite: Option<Cow<'static, str>>,
}

impl CycleButton {
    /// `CycleButton.Builder.create`: the message is `name: value` (`options.generic_value`) or the
    /// value alone.
    pub fn new(ctx_lang: crate::java::text::Lang, x: i32, y: i32, w: i32, h: i32, name: Text, value: Text, display: Display) -> CycleButton {
        let message = match display {
            Display::Value => value,
            _ => option_name_value(ctx_lang, name, value),
        };
        CycleButton { base: Base::new(x, y, w, h, message), display, sprite: None }
    }
}

/// `CommonComponents.optionNameValue`.
pub fn option_name_value(lang: crate::java::text::Lang, name: Text, value: Text) -> Text {
    Text::translate("options.generic_value", &[name, value], lang)
}

impl Widget for CycleButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, hovered: bool) {
        let b = &self.base;
        match &self.sprite {
            Some(s) => ctx.gui.sprite(s.clone(), b.x, b.y, b.w, b.h),
            None => default_sprite(ctx, b, hovered, None),
        }
        if self.display != Display::Hide {
            ctx.scrolling_over(b, b.shown_message(), 2, b.alpha);
        }
    }
}

/// `Checkbox`: the box sprite and up to two lines of label beside it.
#[derive(Clone, Debug)]
pub struct Checkbox {
    pub base: Base,
    pub selected: bool,
    pub text: MultiLineText,
}

/// `Checkbox.getBoxSize`.
pub const BOX: i32 = 9 + 8;

impl Checkbox {
    /// `Checkbox.builder(message, font).maxWidth(max).build()`; `max` defaults to the full width.
    pub fn new(m: &dyn Metrics, x: i32, y: i32, max: Option<i32>, message: Text, selected: bool) -> Checkbox {
        let default = BOX + 4 + m.width(&message);
        let w = max.unwrap_or(default).min(default);
        let mut text = MultiLineText::new(message.clone());
        text.max_rows = Some(2);
        text.max_width = Some(w);
        let h = BOX.max(text.height(m));
        Checkbox { base: Base::new(x, y, w, h, message), selected, text }
    }
}

impl Widget for Checkbox {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn extract(&mut self, ctx: &mut Ctx, _hovered: bool) {
        let b = &self.base;
        let sprite = match (self.selected, b.focused) {
            (true, true) => "widget/checkbox_selected_highlighted",
            (true, false) => "widget/checkbox_selected",
            (false, true) => "widget/checkbox_highlighted",
            (false, false) => "widget/checkbox",
        };
        ctx.gui.sprite_tinted(sprite, b.x, b.y, BOX, BOX, white(b.alpha));
        let h = self.text.height(ctx.font);
        self.text.x = b.x + BOX + 4;
        self.text.y = b.y + BOX / 2 - h / 2;
        self.text.visit_lines(ctx, b.alpha);
    }
}
