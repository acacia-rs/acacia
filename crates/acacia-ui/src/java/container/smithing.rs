//! `SmithingScreen`: the window, the empty slots' cycling icons (`CyclingSlotBackground`), the
//! armor stand preview (a picture-in-picture), the recipe error icon and the onboarding tooltips.

use super::item_combiner::{self, Station};
use super::{Base, Env, Screen, carried, contents, hovered_tooltip};
use crate::java::gui::{Gui, GuiDraw};
use crate::java::mth::white;
use crate::java::picture::Picture;
use crate::java::text::Text;
use crate::java::tooltip::{Line, Positioner, Tooltip};
use crate::java::widget::measure::split;

const TEMPLATE_ICONS: [&str; 2] = ["container/slot/smithing_template_armor_trim", "container/slot/smithing_template_netherite_upgrade"];
const TRIM_BASE: [&str; 4] = ["container/slot/helmet", "container/slot/chestplate", "container/slot/leggings", "container/slot/boots"];
const TRIM_ADDITIONS: [&str; 7] = [
    "container/slot/ingot", "container/slot/redstone_dust", "container/slot/lapis_lazuli", "container/slot/quartz",
    "container/slot/diamond", "container/slot/emerald", "container/slot/amethyst_shard",
];
const UPGRADE_BASE: [&str; 11] = [
    "container/slot/helmet", "container/slot/sword", "container/slot/chestplate", "container/slot/pickaxe", "container/slot/leggings",
    "container/slot/axe", "container/slot/boots", "container/slot/hoe", "container/slot/shovel", "container/slot/nautilus_armor",
    "container/slot/spear",
];
const UPGRADE_ADDITIONS: [&str; 1] = ["container/slot/ingot"];

/// A `SmithingTemplateItem`: an armor trim or the netherite upgrade.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Template {
    Trim,
    Upgrade,
}

impl Template {
    fn of(id: &str) -> Option<Template> {
        match id {
            "minecraft:netherite_upgrade_smithing_template" => Some(Template::Upgrade),
            _ if id.ends_with("_armor_trim_smithing_template") => Some(Template::Trim),
            _ => None,
        }
    }

    fn icons(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            Template::Trim => (&TRIM_BASE, &TRIM_ADDITIONS),
            Template::Upgrade => (&UPGRADE_BASE, &UPGRADE_ADDITIONS),
        }
    }

    fn description(self, slot: usize) -> String {
        let kind = if self == Template::Trim { "armor_trim" } else { "netherite_upgrade" };
        let part = if slot == 1 { "base_slot_description" } else { "additions_slot_description" };
        format!("item.minecraft.smithing_template.{kind}.{part}")
    }
}

pub struct Smithing {
    pub base: Base,
    /// Data: whether the inputs match no recipe.
    pub station: Station,
    /// `containerTick`s since the screen opened.
    pub ticks: i32,
}

impl Smithing {
    pub fn new(width: i32, height: i32, title: Text, inventory: Text, station: Station, ticks: i32) -> Smithing {
        let slots = item_combiner::slots(&[(8, 48), (26, 48), (44, 48), (98, 48)], (8, 84));
        let mut base = Base::new(width, height, [176, 166], title, inventory, slots);
        base.title_label = [44, 15];
        Smithing { base, station, ticks }
    }

    fn template(&self) -> Option<Template> {
        self.base.slots[0].item.as_ref().filter(|s| !s.is_empty()).and_then(|s| Template::of(&s.id))
    }

    fn error(&self) -> bool {
        self.station.data(0) > 0
    }

    /// `CyclingSlotBackground.extractRenderState` for `slot` after `ticks` ticks with `icons`.
    fn cycling(&self, gui: &mut Gui, slot: usize, icons: &[&'static str], partial: f32) {
        let s = &self.base.slots[slot];
        // The icons are only set by the first tick.
        if icons.is_empty() || self.ticks == 0 || s.has_item() {
            return;
        }
        let (tick, len) = (self.ticks, icons.len() as i32);
        let index = (tick / 30 % len) as usize;
        let alpha = if len > 1 && tick >= 30 { ((tick % 30) as f32 + partial).min(4.0) / 4.0 } else { 1.0 };
        let (x, y) = (self.base.left + s.x, self.base.top + s.y);
        if alpha < 1.0 {
            gui.sprite_tinted(icons[(index as i32 - 1).rem_euclid(len) as usize], x, y, 16, 16, white(1.0 - alpha));
        }
        gui.sprite_tinted(icons[index], x, y, 16, 16, white(alpha));
    }

    fn onboarding(&mut self, env: &Env, mouse: [i32; 2]) {
        let mut tooltip = None;
        if self.error() && self.hovering(65, 46, 28, 21, mouse) {
            tooltip = Some("container.upgrade.error_tooltip".to_owned());
        }
        if let Some(hovered) = self.base.hovered.map(|i| &self.base.slots[i]) {
            match self.template() {
                _ if !self.base.slots[0].has_item() => {
                    if hovered.index == 0 {
                        tooltip = Some("container.upgrade.missing_template_tooltip".to_owned());
                    }
                }
                Some(t) if !hovered.has_item() && (hovered.index == 1 || hovered.index == 2) => tooltip = Some(t.description(hovered.index)),
                _ => {}
            }
        }
        if let Some(key) = tooltip {
            let lines = split(env.font, &Text::translate(&key, &[], env.lang), 115).into_iter().map(Line::Text).collect();
            let t = Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style: None, extra_space: false };
            self.base.tooltip.set(t, false);
        }
    }
}

impl Screen for Smithing {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn background(&mut self, gui: &mut Gui, _env: &Env, _mouse: [i32; 2], partial: f32) {
        let b = &self.base;
        item_combiner::background(gui, b, "gui/container/smithing");
        if self.error() {
            gui.sprite("container/smithing/error", b.left + 65, b.top + 46, 28, 21);
        }
        let (bases, additions) = self.template().map_or((&[][..], &[][..]), Template::icons);
        self.cycling(gui, 0, &TEMPLATE_ICONS, partial);
        self.cycling(gui, 1, bases, partial);
        self.cycling(gui, 2, additions, partial);
        let (x0, y0) = (self.base.left + 121, self.base.top + 20);
        // Quaternionf().rotationXYZ(25 degrees, 0, PI).
        let (sx, cx) = (0.436_332_32f32 * 0.5).sin_cos();
        let (sz, cz) = (std::f32::consts::PI * 0.5).sin_cos();
        let rotation = [sx * cz, -sx * sz, cx * sz, cx * cz];
        gui.draws.push(GuiDraw::Picture(Picture::Entity {
            entity: "minecraft:armor_stand".to_owned(),
            scale: 25.0,
            translation: [0.0, 1.0, 0.0],
            rotation,
            camera: None,
            angles: Some([210.0, 0.0, 25.0]),
            rect: [x0, y0, self.base.left + 161, self.base.top + 80],
        }));
    }

    fn render(&mut self, gui: &mut Gui, env: &Env, mouse: [i32; 2], partial: f32) {
        contents(self, gui, env, mouse, partial);
        carried(&self.base, gui, mouse);
        hovered_tooltip(self, env, mouse);
        self.onboarding(env, mouse);
    }
}
