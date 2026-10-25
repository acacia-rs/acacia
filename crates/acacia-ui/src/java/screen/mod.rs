//! Java 26.3's non-container screens (`net.minecraft.client.gui.screens`): each builds its widgets
//! as its `init()` does and draws as `Screen.extractRenderStateWithTooltipAndSubtitles` does —
//! a stratum, the background, a stratum, the widgets and whatever the screen adds.

pub mod book;
pub mod book_edit;
pub mod confirm;
pub mod death;
pub mod message;
pub mod options;
pub mod pause;
pub mod sign;
pub mod telemetry;

#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod text_tests;

use crate::java::widget::input::{self, Child, Fired, Interaction, Key};
use crate::java::widget::layout::Arena;
use crate::java::widget::{Ctx, Widget, render};

/// What every screen reads from the client.
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub width: i32,
    pub height: i32,
    /// `minecraft.level != null`: the in-world menu textures, and no panorama.
    pub in_world: bool,
    /// `options.getMenuBackgroundBlurriness() >= 1`.
    pub blur: bool,
}

/// What Escape does to a screen (`keyPressed` before its children see the key).
#[derive(Clone, Debug, PartialEq)]
pub enum Escape {
    /// `shouldCloseOnEsc`: `onClose`.
    Close,
    Ignore,
    /// The screen answers it, as a `ConfirmScreen` says no.
    Fire(Fired),
}

/// A screen: its background, its foreground and the input its children take (see
/// `widget::input`; the owner keeps the [`Interaction`] and restores it into each rebuild).
pub trait Screen {
    /// `extractBackground`.
    fn background(&mut self, ctx: &mut Ctx);
    /// `extractRenderState`.
    fn foreground(&mut self, ctx: &mut Ctx);
    /// The children input goes to, in order.
    fn children(&mut self) -> Vec<Child<'_>> {
        Vec::new()
    }
    fn escape(&self) -> Escape {
        Escape::Close
    }
    /// The focus (and scroll) a rebuilt screen had.
    fn restore(&mut self, s: &Interaction) {
        input::restore(&mut self.children(), s);
    }
    fn mouse_clicked(&mut self, s: &mut Interaction, x: f64, y: f64, shift: bool) -> Option<Fired> {
        input::mouse_clicked(&mut self.children(), s, x, y, shift)
    }
    fn mouse_released(&mut self, s: &mut Interaction) {
        input::mouse_released(&mut self.children(), s);
    }
    /// The mouse moved to (`x`, `y`) by `dy` with the button held.
    fn mouse_dragged(&mut self, s: &mut Interaction, x: f64, _y: f64, _dy: f64) -> Option<Fired> {
        input::mouse_dragged(&mut self.children(), s, x)
    }
    /// The wheel turned `amount` notches (up is positive).
    fn mouse_scrolled(&mut self, _s: &mut Interaction, _x: f64, _y: f64, _amount: f64) -> bool {
        false
    }
    fn key_pressed(&mut self, s: &mut Interaction, key: Key, shift: bool) -> Option<Fired> {
        input::key_pressed(&mut self.children(), s, key, shift)
    }
}

/// Every widget of `widgets` as a child.
pub fn arena_children(widgets: &mut [Box<dyn Widget>]) -> Vec<Child<'_>> {
    widgets.iter_mut().map(|w| Child { widget: w.as_mut(), clip: None }).collect()
}

/// `extractRenderStateWithTooltipAndSubtitles` without the deferred tooltips.
pub fn extract(screen: &mut dyn Screen, ctx: &mut Ctx) {
    ctx.gui.next_stratum();
    screen.background(ctx);
    ctx.gui.next_stratum();
    screen.foreground(ctx);
}

/// `Screen.extractRenderState`: the renderables in the order they were added.
pub fn render_all(widgets: &mut Arena, ctx: &mut Ctx) {
    for w in widgets.iter_mut() {
        render(w.as_mut(), ctx);
    }
}

/// `Screen.extractBackground` for a menu screen (the panorama is not drawn: see `Env::in_world`).
pub fn menu_background(env: &Env, ctx: &mut Ctx) {
    blurred(env, ctx);
    menu_texture(env, ctx, 0, 0, env.width, env.height);
}

/// `extractBlurredBackground`.
pub fn blurred(env: &Env, ctx: &mut Ctx) {
    if env.blur {
        ctx.gui.blur();
    }
}

/// `extractMenuBackground(x, y, w, h)`: the 32x32 menu texture tiled.
pub fn menu_texture(env: &Env, ctx: &mut Ctx, x: i32, y: i32, w: i32, h: i32) {
    let texture = if env.in_world { "gui/inworld_menu_background" } else { "gui/menu_background" };
    ctx.blit(texture, x, y, [0.0, 0.0], w, h, [32, 32]);
}

/// `extractTransparentBackground`: the in-game dim.
pub fn transparent_background(env: &Env, ctx: &mut Ctx) {
    ctx.gui.gradient(0, 0, env.width, env.height, 0xC010_1010, 0xD010_1010);
}
