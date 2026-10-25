//! `BossHealthOverlay.extractRenderState` and `JumpableVehicleBar.extractBackground`. Ported from
//! Horizon's `hud_overlays.rs` (MIT).

use crate::java::gui::{Gui, Metrics};
use crate::java::mth::{WHITE, lerp_discrete};
use crate::java::text::Text;

/// One `LerpingBossEvent`.
#[derive(Clone, Debug, PartialEq)]
pub struct BossBar {
    pub name: Text,
    /// `getProgress()`, the lerped value.
    pub progress: f32,
    /// `BossBarColor` ordinal.
    pub color: usize,
    /// `BossBarOverlay` ordinal; 0 is `PROGRESS`, drawn without an overlay.
    pub overlay: usize,
}

const COLORS: [&str; 7] = ["pink", "blue", "red", "green", "yellow", "purple", "white"];
const OVERLAYS: [&str; 5] = ["progress", "notched_6", "notched_10", "notched_12", "notched_20"];

fn boss_part(gui: &mut Gui, bar: &BossBar, x: i32, y: i32, width: i32, part: &str) {
    gui.region(format!("boss_bar/{}_{part}", COLORS[bar.color]), [182, 5], [0, 0], x, y, width, 5);
    if bar.overlay != 0 {
        gui.region(format!("boss_bar/{}_{part}", OVERLAYS[bar.overlay]), [182, 5], [0, 0], x, y, width, 5);
    }
}

/// `BossHealthOverlay.extractRenderState`: bars in event order 19 pixels apart from y = 12, until
/// a third of the screen height.
pub fn boss_overlay(gui: &mut Gui, font: &dyn Metrics, bars: &[BossBar]) {
    let mut y = 12;
    for bar in bars {
        let x = gui.width / 2 - 91;
        boss_part(gui, bar, x, y, 182, "background");
        let progress = lerp_discrete(bar.progress, 0, 182);
        if progress > 0 {
            boss_part(gui, bar, x, y, progress, "progress");
        }
        let width = font.width(&bar.name);
        gui.text(bar.name.clone(), gui.width / 2 - width / 2, y - 9, WHITE, true);
        y += 10 + 9;
        if y >= gui.height / 3 {
            break;
        }
    }
}

/// `JumpableVehicleBar.extractBackground`: `cooldown` is the mount's `getJumpCooldown`, `scale`
/// the player's `getJumpRidingScale`.
pub fn jump_bar(gui: &mut Gui, cooldown: i32, scale: f32) {
    let (left, top) = ((gui.width - 182) / 2, gui.height - 24 - 5);
    gui.sprite("hud/jump_bar_background", left, top, 182, 5);
    if cooldown > 0 {
        gui.sprite("hud/jump_bar_cooldown", left, top, 182, 5);
    } else {
        let progress = lerp_discrete(scale, 0, 182);
        if progress > 0 {
            gui.region("hud/jump_bar_progress", [182, 5], [0, 0], left, top, progress, 5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::gui::{Fixed, GuiDraw};

    #[test]
    fn bars_stop_at_a_third_of_the_screen() {
        let bar = BossBar { name: Text::literal("Boss"), progress: 0.5, color: 2, overlay: 1 };
        let mut gui = Gui::new(320, 90);
        boss_overlay(&mut gui, &Fixed(6.0), &[bar.clone(), bar]);
        assert_eq!(gui.draws.len(), 5);
        assert!(matches!(&gui.draws[2], GuiDraw::Sprite { sprite, w: 91, .. } if sprite == "boss_bar/red_progress"));
    }

    #[test]
    fn jump_bar_fills_by_scale() {
        let mut gui = Gui::new(320, 240);
        jump_bar(&mut gui, 0, 0.0);
        assert_eq!(gui.draws.len(), 1);
        jump_bar(&mut gui, 0, 1.0);
        assert!(matches!(&gui.draws[2], GuiDraw::Sprite { w: 182, region: Some(_), .. }));
    }
}
