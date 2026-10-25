//! `Hud.extractCameraOverlays` (vignette and border warning, spyglass, equipment overlays such as
//! the pumpkin blur, powder snow, nether portal, nausea) and `extractSleepOverlay`. Ported from
//! Horizon's `hud_overlays.rs` (MIT).

use crate::java::gui::{Blend, Gui, GuiDraw, PoseOp};
use crate::java::mth::{BLACK, WHITE, clamp, color_from_float, floor, lerp, white};

/// A full-screen `blit` of `textures/<texture>.png`.
fn screen_blit(gui: &mut Gui, texture: impl Into<std::borrow::Cow<'static, str>>, blend: Blend, color: u32) {
    let (w, h) = (gui.width, gui.height);
    gui.draws.push(GuiDraw::Blit { texture: texture.into(), blend, x: 0, y: 0, w, h, uv: [0.0, 0.0], src: [w, h], tex: [w, h], color });
}

/// `extractVignette`'s border warning strength for a camera `distance` from the border
/// (`getDistanceToBorder`), its warning blocks and time, `getLerpSpeed` and how far its size still
/// moves (`getLerpTarget - getSize`).
pub fn border_warning(distance: f32, warning_blocks: i32, warning_time: i32, lerp_speed: f64, size_left: f64) -> f32 {
    let moving = (lerp_speed * f64::from(warning_time)).min(size_left.abs());
    let warning = f64::from(warning_blocks).max(moving);
    if f64::from(distance) < warning { 1.0 - (f64::from(distance) / warning) as f32 } else { 0.0 }
}

/// `Hud.extractVignette`: `brightness` is `vignetteBrightness`, `warning` per [`border_warning`].
pub fn vignette(gui: &mut Gui, brightness: f32, warning: f32) {
    let brightness = clamp(brightness, 0.0, 1.0);
    let color = if warning > 0.0 {
        let warning = clamp(warning, 0.0, 1.0);
        let green_blue = brightness + (1.0 - brightness) * warning;
        color_from_float(1.0, brightness * (1.0 - warning), green_blue, green_blue)
    } else {
        color_from_float(1.0, brightness, brightness, brightness)
    };
    screen_blit(gui, "misc/vignette", Blend::Vignette, color);
}

/// `Hud.extractSpyglassOverlay`.
pub fn spyglass(gui: &mut Gui, scale: f32) {
    let (w, h) = (gui.width, gui.height);
    let src = w.min(h) as f32;
    let ratio = (w as f32 / src).min(h as f32 / src) * scale;
    let size = floor(src * ratio);
    let (left, top) = ((w - size) / 2, (h - size) / 2);
    let (right, bottom) = (left + size, top + size);
    gui.draws.push(GuiDraw::Blit {
        texture: "misc/spyglass_scope".into(),
        blend: Blend::Textured,
        x: left,
        y: top,
        w: size,
        h: size,
        uv: [0.0, 0.0],
        src: [size, size],
        tex: [size, size],
        color: WHITE,
    });
    gui.fill(0, bottom, w, h, BLACK);
    gui.fill(0, 0, w, top, BLACK);
    gui.fill(0, top, left, bottom, BLACK);
    gui.fill(right, top, w, bottom, BLACK);
}

/// `Hud.extractPortalOverlay`: the nether portal's particle sprite over the screen.
pub fn portal(gui: &mut Gui, mut alpha: f32) {
    if alpha < 1.0 {
        alpha *= alpha;
        alpha *= alpha;
        alpha = alpha * 0.8 + 0.2;
    }
    let (w, h) = (gui.width, gui.height);
    gui.draws.push(GuiDraw::AtlasSprite { sprite: "block/nether_portal".into(), x: 0, y: 0, w, h, color: white(alpha) });
}

/// `Hud.extractConfusionOverlay`: the nausea swirl scaled about the centre.
pub fn confusion(gui: &mut Gui, strength: f32) {
    let size = lerp(strength, 2.0, 1.0);
    let (cx, cy) = (gui.width as f32 / 2.0, gui.height as f32 / 2.0);
    gui.push(vec![PoseOp::Translate(cx, cy), PoseOp::Scale(size, size), PoseOp::Translate(-cx, -cy)]);
    screen_blit(gui, "misc/nausea", Blend::Nausea, color_from_float(1.0, 0.2 * strength, 0.4 * strength, 0.2 * strength));
    gui.pop();
}

/// What `Hud.extractCameraOverlays` reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraOverlays {
    /// `Options.vignette` on: `vignetteBrightness` and the border warning strength.
    pub vignette: Option<(f32, f32)>,
    pub first_person: bool,
    /// `LocalPlayer.isScoping`.
    pub scoping: bool,
    /// `DeltaTracker.getGameTimeDeltaTicks`.
    pub game_time_delta: f32,
    /// The worn items' `Equippable.cameraOverlay` textures (`misc/pumpkinblur`), in
    /// `EquipmentSlot` order.
    pub equipment: Vec<String>,
    /// `getTicksFrozen` and `getPercentFrozen`.
    pub ticks_frozen: i32,
    pub percent_frozen: f32,
    pub partial: f32,
    /// `oPortalEffectIntensity` and `portalEffectIntensity`.
    pub portal: [f32; 2],
    /// `getEffectBlendFactor(NAUSEA, partial)`.
    pub nausea: f32,
    /// `Options.screenEffectScale`.
    pub screen_effect_scale: f32,
}

/// `Hud.extractCameraOverlays`, easing `scope_scale` (`Hud.scopeScale`) as it does.
pub fn camera_overlays(gui: &mut Gui, i: &CameraOverlays, scope_scale: &mut f32) {
    if let Some((brightness, warning)) = i.vignette {
        vignette(gui, brightness, warning);
    }
    *scope_scale = lerp(0.5 * i.game_time_delta, *scope_scale, 1.125);
    if i.first_person {
        if i.scoping {
            spyglass(gui, *scope_scale);
        } else {
            *scope_scale = 0.5;
            for texture in &i.equipment {
                screen_blit(gui, texture.clone(), Blend::Textured, WHITE);
            }
        }
    }
    if i.ticks_frozen > 0 {
        screen_blit(gui, "misc/powder_snow_outline", Blend::Textured, white(i.percent_frozen));
    }
    let intensity = lerp(i.partial, i.portal[0], i.portal[1]);
    if intensity > 0.0 {
        portal(gui, intensity);
    } else if i.nausea > 0.0 && i.screen_effect_scale < 1.0 {
        confusion(gui, i.nausea * (1.0 - i.screen_effect_scale));
    }
}

/// `Hud.extractSleepOverlay`.
pub fn sleep_overlay(gui: &mut Gui, sleep_timer: i32) {
    if sleep_timer <= 0 {
        return;
    }
    let timer = sleep_timer as f32;
    let mut amount = timer / 100.0;
    if amount > 1.0 {
        amount = 1.0 - (timer - 100.0) / 10.0;
    }
    let (w, h) = (gui.width, gui.height);
    gui.fill(0, 0, w, h, (((220.0 * amount) as i32) << 24) as u32 | 0x10_1020);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_ramps_inside_the_warning_distance() {
        assert_eq!(border_warning(10.0, 5, 15, 0.0, 0.0), 0.0);
        assert_eq!(border_warning(2.5, 5, 15, 0.0, 0.0), 0.5);
    }

    #[test]
    fn scope_eases_while_scoping_and_resets_otherwise() {
        let mut gui = Gui::new(320, 240);
        let mut scale = 0.5;
        camera_overlays(&mut gui, &CameraOverlays { first_person: true, scoping: true, game_time_delta: 1.0, ..Default::default() }, &mut scale);
        assert_eq!(scale, 0.8125);
        assert_eq!(gui.draws.len(), 5);
        camera_overlays(&mut gui, &CameraOverlays { first_person: true, ..Default::default() }, &mut scale);
        assert_eq!(scale, 0.5);
    }

    #[test]
    fn sleep_fades_out_after_100_ticks() {
        let mut gui = Gui::new(10, 10);
        sleep_overlay(&mut gui, 105);
        assert_eq!(gui.draws[0], GuiDraw::Fill { x0: 0, y0: 0, x1: 10, y1: 10, color: 0x6E10_1020 });
    }
}
