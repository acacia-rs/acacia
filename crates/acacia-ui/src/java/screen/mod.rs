//! Java 26.3's non-container screens (`net.minecraft.client.gui.screens`): each builds its widgets
//! as its `init()` does and draws as `Screen.extractRenderStateWithTooltipAndSubtitles` does —
//! a stratum, the background, a stratum, the widgets and whatever the screen adds.

pub mod confirm;
pub mod death;
pub mod message;
pub mod options;
pub mod pause;

use crate::java::widget::layout::Arena;
use crate::java::widget::{Ctx, render};

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

/// A screen: its background and its foreground.
pub trait Screen {
    /// `extractBackground`.
    fn background(&mut self, ctx: &mut Ctx);
    /// `extractRenderState`.
    fn foreground(&mut self, ctx: &mut Ctx);
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
