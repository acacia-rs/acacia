//! `Hud.extractItemHotbar`, `extractCrosshair`, `ExperienceBar` and `extractVehicleHealth`.

use crate::java::gui::{Blend, Gui, Metrics, PoseOp};
use crate::java::text::Text;

/// `Hud.extractItemHotbar`'s inputs.
#[derive(Debug, Clone, Default)]
pub struct Hotbar {
    pub selected: i32,
    /// The main arm is the right one, so the off hand sits left of the bar.
    pub right_handed: bool,
    /// Per hotbar slot holding an item: its pop time less the partial tick (`ItemStack.getPopTime`).
    pub slots: [Option<f32>; 9],
    /// The same for the off hand, when it holds an item.
    pub offhand: Option<f32>,
    /// `getAttackStrengthScale(0)` while the attack indicator is in HOTBAR mode.
    pub attack: Option<f32>,
}

/// The hotbar's items carry their slot as `key`; the off hand is 40.
pub const OFFHAND_KEY: i32 = 40;

pub fn hotbar(gui: &mut Gui, i: &Hotbar) {
    let (center, h) = (gui.width / 2, gui.height);
    gui.sprite("hud/hotbar", center - 91, h - 22, 182, 22);
    gui.sprite("hud/hotbar_selection", center - 91 - 1 + i.selected * 20, h - 22 - 1, 24, 23);
    let offhand_left = i.right_handed;
    if i.offhand.is_some() {
        if offhand_left {
            gui.sprite("hud/hotbar_offhand_left", center - 91 - 29, h - 23, 29, 24);
        } else {
            gui.sprite("hud/hotbar_offhand_right", center + 91, h - 23, 29, 24);
        }
    }
    let mut seed = 1;
    for (n, pop) in i.slots.iter().enumerate() {
        if let Some(pop) = pop {
            slot(gui, n as i32, center - 90 + n as i32 * 20 + 2, h - 16 - 3, seed, *pop);
        }
        seed += 1;
    }
    if let Some(pop) = i.offhand {
        let x = if offhand_left { center - 91 - 26 } else { center + 91 + 10 };
        slot(gui, OFFHAND_KEY, x, h - 16 - 3, seed, pop);
    }
    if let Some(scale) = i.attack.filter(|&s| s < 1.0) {
        let y = h - 20;
        let x = if offhand_left { center + 91 + 6 } else { center - 91 - 22 };
        let progress = (scale * 19.0) as i32;
        gui.sprite("hud/hotbar_attack_indicator_background", x, y, 18, 18);
        gui.region("hud/hotbar_attack_indicator_progress", [18, 18], [0, 18 - progress], x, y + 18 - progress, 18, progress);
    }
}

/// `Hud.extractSlot`: an item squeezed while it pops in.
fn slot(gui: &mut Gui, key: i32, x: i32, y: i32, seed: i32, pop: f32) {
    if pop > 0.0 {
        let squeeze = 1.0 + pop / 5.0;
        let (px, py) = ((x + 8) as f32, (y + 12) as f32);
        gui.push(vec![PoseOp::Translate(px, py), PoseOp::Scale(1.0 / squeeze, (squeeze + 1.0) / 2.0), PoseOp::Translate(-px, -py)]);
        gui.item(key, x, y, seed);
        gui.pop();
    } else {
        gui.item(key, x, y, seed);
    }
    gui.decorations(key, x, y, None);
}

/// Whether `extractCrosshair` shows the full attack indicator: at full strength on a living, alive
/// `crosshairPickEntity` (`target`) with a weapon slower than `MIN_CROSSHAIR_ATTACK_SPEED`
/// (`delay` is `getCurrentItemAttackStrengthDelay`).
pub fn full_attack_indicator(scale: f32, delay: f32, target: Option<bool>) -> bool {
    scale >= 1.0 && delay > 5.0 && target == Some(true)
}

/// `Hud.extractCrosshair` in first person, every sprite through the inverting pipeline: `attack`
/// is `getAttackStrengthScale(0)` in CROSSHAIR indicator mode, `full` per [`full_attack_indicator`].
pub fn crosshair(gui: &mut Gui, attack: Option<f32>, full: bool) {
    let (w, h) = (gui.width, gui.height);
    gui.next_stratum();
    let start = gui.draws.len();
    gui.sprite("hud/crosshair", (w - 15) / 2, (h - 15) / 2, 15, 15);
    if let Some(scale) = attack {
        let (x, y) = (w / 2 - 8, h / 2 - 7 + 16);
        if full {
            gui.sprite("hud/crosshair_attack_indicator_full", x, y, 16, 16);
        } else if scale < 1.0 {
            let progress = (scale * 17.0) as i32;
            gui.sprite("hud/crosshair_attack_indicator_background", x, y, 16, 4);
            gui.region("hud/crosshair_attack_indicator_progress", [16, 4], [0, 0], x, y, progress, 4);
        }
    }
    for d in &mut gui.draws[start..] {
        if let crate::java::gui::GuiDraw::Sprite { blend, .. } = d {
            *blend = Blend::Crosshair;
        }
    }
}

/// `ExperienceBar.extractBackground` (when `bar`) and `ContextualBar.extractExperienceLevel`.
pub fn experience(gui: &mut Gui, font: &dyn Metrics, progress: f32, level: i32, bar: bool) {
    let (w, h) = (gui.width, gui.height);
    if bar {
        let (left, top) = (w / 2 - 91, h - 32 + 3);
        let filled = (progress * 183.0) as i32;
        gui.sprite("hud/experience_bar_background", left, top, 182, 5);
        if filled > 0 {
            gui.region("hud/experience_bar_progress", [182, 5], [0, 0], left, top, filled, 5);
        }
    }
    if level > 0 {
        let text = Text::literal(level.to_string());
        let x = (w - font.width(&text)) / 2;
        let y = h - 24 - 9 - 2;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            gui.text(text.clone(), x + dx, y + dy, 0xFF00_0000, false);
        }
        gui.text(text, x, y, 0xFF80_FF20, false);
    }
}

/// `Hud.getVehicleMaxHearts` for a vehicle that shows its health.
pub fn vehicle_max_hearts(max_health: f32) -> i32 {
    (((max_health + 0.5) as i32) / 2).min(30)
}

/// `Hud.extractVehicleHealth`.
pub fn vehicle_health(gui: &mut Gui, hearts: i32, health: f32) {
    let current = f64::from(health).ceil() as i32;
    let x_right = gui.width / 2 + 91;
    let mut y = gui.height - 39;
    let (mut hearts, mut base) = (hearts, 0);
    while hearts > 0 {
        let row = hearts.min(10);
        hearts -= row;
        for i in 0..row {
            let x = x_right - i * 8 - 9;
            gui.sprite("hud/heart/vehicle_container", x, y, 9, 9);
            if i * 2 + 1 + base < current {
                gui.sprite("hud/heart/vehicle_full", x, y, 9, 9);
            }
            if i * 2 + 1 + base == current {
                gui.sprite("hud/heart/vehicle_half", x, y, 9, 9);
            }
        }
        y -= 10;
        base += 20;
    }
}
