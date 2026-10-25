//! `BeaconScreen`: the window with its payment items, the effect labels, the power buttons (one
//! per effect of each unlocked tier, the secondary's level II upgrade once a primary is chosen),
//! confirm and cancel, and the hovered button's tooltip.

use super::item_combiner::{self, Station};
use super::{Base, Env, Screen, blit, transparent_background};
use crate::java::gui::Gui;
use crate::java::text::Text;
use crate::java::tooltip::{Line, Positioner, Tooltip};
use crate::java::widget::measure::split;

/// `BeaconBlockEntity.BEACON_EFFECTS` by tier.
const EFFECTS: [&[&str]; 4] = [&["speed", "haste"], &["resistance", "jump_boost"], &["strength"], &["regeneration"]];
const LABEL: u32 = 0xFFE0_E0E0;

enum Kind {
    /// An effect of `tier`, primary or not; the upgrade is the secondary row's level II.
    Power { effect: String, primary: bool, tier: i32, upgrade: bool },
    Confirm,
    Cancel,
}

struct Button {
    kind: Kind,
    x: i32,
    y: i32,
    active: bool,
    selected: bool,
    visible: bool,
}

impl Button {
    fn power(x: i32, y: i32, effect: &str, primary: bool, tier: i32) -> Button {
        let kind = Kind::Power { effect: effect.to_owned(), primary, tier, upgrade: false };
        Button { kind, x, y, active: false, selected: false, visible: true }
    }

    fn message(&self, env: &Env) -> Text {
        match &self.kind {
            Kind::Power { effect, upgrade, .. } => {
                let name = Text::translate(&format!("effect.minecraft.{effect}"), &[], env.lang);
                if *upgrade { name.append(" II") } else { name }
            }
            Kind::Confirm => Text::translate("gui.done", &[], env.lang),
            Kind::Cancel => Text::translate("gui.cancel", &[], env.lang),
        }
    }

    fn icon(&self) -> String {
        match &self.kind {
            Kind::Power { effect, .. } => format!("mob_effect/{effect}"),
            Kind::Confirm => "container/beacon/confirm".to_owned(),
            Kind::Cancel => "container/beacon/cancel".to_owned(),
        }
    }
}

pub struct Beacon {
    pub base: Base,
    buttons: Vec<Button>,
    /// The chosen effects by path (`speed`).
    pub primary: Option<String>,
    pub secondary: Option<String>,
    levels: i32,
    payment: bool,
}

/// What a press on the beacon's buttons did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BeaconPress {
    /// A power chosen: `primary` and `secondary` hold the choice.
    Chose,
    /// Confirm: `ServerboundSetBeaconPacket` with the choice, then the screen closes.
    Confirm,
    Cancel,
}

/// The draw keys of the payment items under the slot (emerald, diamond, gold, iron, netherite)
/// for a painter: `PAYMENT + i`. The oracle keys them -1.
pub const PAYMENT: i32 = -1100;

impl Beacon {
    /// The screen after a tick: buttons as `updateStatus(levels)` leaves them for the chosen
    /// `primary` and `secondary` effects (by path, `speed`), the payment slot filled or not.
    #[allow(clippy::too_many_arguments)]
    pub fn new(width: i32, height: i32, title: Text, inventory: Text, station: &Station, primary: Option<&str>, secondary: Option<&str>, payment: bool) -> Beacon {
        let base = Base::new(width, height, [230, 219], title, inventory, item_combiner::slots(&[(136, 110)], (36, 137)));
        let (left, top) = (base.left, base.top);
        let mut buttons = Vec::new();
        for (tier, effects) in EFFECTS[..3].iter().enumerate() {
            let total = effects.len() as i32 * 24 - 2;
            for (c, effect) in effects.iter().enumerate() {
                buttons.push(Button::power(left + 76 + c as i32 * 24 - total / 2, top + 22 + tier as i32 * 25, effect, true, tier as i32));
            }
        }
        let total = 2 * 24 - 2;
        buttons.push(Button::power(left + 167 - total / 2, top + 47, EFFECTS[3][0], false, 3));
        let mut upgrade = Button::power(left + 167 + 24 - total / 2, top + 47, EFFECTS[0][0], false, 3);
        upgrade.kind = Kind::Power { effect: EFFECTS[0][0].to_owned(), primary: false, tier: 3, upgrade: true };
        buttons.push(upgrade);
        buttons.push(Button { kind: Kind::Confirm, x: left + 164, y: top + 107, active: true, selected: false, visible: true });
        buttons.push(Button { kind: Kind::Cancel, x: left + 190, y: top + 107, active: true, selected: false, visible: true });
        let (primary, secondary) = (primary.map(str::to_owned), secondary.map(str::to_owned));
        let mut beacon = Beacon { base, buttons, primary, secondary, levels: station.data(0), payment };
        beacon.update_buttons();
        beacon
    }

    /// `updateButtons`: each button's `updateStatus(levels)`.
    fn update_buttons(&mut self) {
        let (primary, secondary, levels) = (self.primary.as_deref(), self.secondary.as_deref(), self.levels);
        for b in &mut self.buttons {
            match &mut b.kind {
                Kind::Power { upgrade: true, effect, tier, .. } => {
                    b.visible = primary.is_some();
                    if let Some(p) = primary {
                        *effect = p.to_owned();
                        b.active = *tier < levels;
                        b.selected = Some(p) == secondary;
                    }
                }
                Kind::Power { effect, primary: is_primary, tier, .. } => {
                    b.active = *tier < levels;
                    b.selected = Some(effect.as_str()) == if *is_primary { primary } else { secondary };
                }
                Kind::Confirm => b.active = self.payment && primary.is_some(),
                Kind::Cancel => {}
            }
        }
    }

    /// `mouseClicked` on the buttons (left only): a power's `onPress`, confirm or cancel.
    pub fn mouse_clicked(&mut self, mouse: [f64; 2]) -> Option<BeaconPress> {
        let hit = |b: &Button| b.visible && b.active && mouse[0] >= f64::from(b.x) && mouse[1] >= f64::from(b.y) && mouse[0] < f64::from(b.x + 22) && mouse[1] < f64::from(b.y + 22);
        let b = self.buttons.iter().find(|b| hit(b))?;
        let (effect, primary) = match &b.kind {
            Kind::Confirm => return Some(BeaconPress::Confirm),
            Kind::Cancel => return Some(BeaconPress::Cancel),
            Kind::Power { .. } if b.selected => return Some(BeaconPress::Chose),
            Kind::Power { effect, primary, .. } => (effect.clone(), *primary),
        };
        if primary {
            if self.secondary.as_ref() != Some(&effect) {
                self.secondary = None;
            }
            self.primary = Some(effect);
        } else {
            self.secondary = Some(effect);
        }
        self.update_buttons();
        Some(BeaconPress::Chose)
    }
}

impl Screen for Beacon {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], _partial: f32) {
        transparent_background(gui);
        let b = &self.base;
        let (xo, yo) = ((b.width - b.image_width) / 2, (b.height - b.image_height) / 2);
        blit(gui, "gui/container/beacon", xo, yo, [0.0, 0.0], b.image_width, b.image_height);
        for (i, x) in [20, 41, 41 + 22, 42 + 44, 42 + 66].into_iter().enumerate() {
            gui.item(if b.distinct_keys { PAYMENT + i as i32 } else { -1 }, xo + x, yo + 109, 0);
        }
    }

    fn widgets(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], _partial: f32) {
        for b in self.buttons.iter().filter(|b| b.visible) {
            let hovered = mouse[0] >= b.x && mouse[1] >= b.y && mouse[0] < b.x + 22 && mouse[1] < b.y + 22;
            let sprite = match (b.active, b.selected, hovered) {
                (false, ..) => "container/beacon/button_disabled",
                (_, true, _) => "container/beacon/button_selected",
                (_, _, true) => "container/beacon/button_highlighted",
                _ => "container/beacon/button",
            };
            gui.sprite(sprite, b.x, b.y, 22, 22);
            gui.sprite(b.icon(), b.x + 2, b.y + 2, 18, 18);
            if hovered {
                let lines = split(env.font, &b.message(env), 170).into_iter().map(Line::Text).collect();
                let positioner = Positioner::Menu([b.x, b.y, 22, 22]);
                self.base.tooltip.set(Tooltip { lines, x: mouse[0], y: mouse[1], positioner, style: None, extra_space: false }, false);
            }
        }
    }

    fn labels(&mut self, gui: &mut Gui, env: &Env, _mouse: [i32; 2]) {
        for (key, x) in [("block.minecraft.beacon.primary", 62), ("block.minecraft.beacon.secondary", 169)] {
            let text = Text::translate(key, &[], env.lang);
            let w = env.font.width(&text);
            gui.text(text, x - w / 2, 10, LABEL, true);
        }
    }
}
