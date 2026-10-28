//! `EffectsInInventory`: the active effects beside the inventory and creative windows, each a
//! background with the effect's icon, name and time left, clipped with a tooltip when narrow.

use std::cmp::Ordering;

use super::Base;
use crate::java::gui::{Gui, Metrics};
use crate::java::mth::floor;
use crate::java::text::{Lang, Text};
use crate::java::tooltip::{Line, Positioner, Tooltip};

/// A `MobEffectInstance` and its effect's colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    /// `minecraft:speed`.
    pub id: String,
    /// Ticks; -1 is infinite.
    pub duration: i32,
    pub amplifier: i32,
    pub ambient: bool,
    /// `MobEffect.getColor`.
    pub color: i32,
}

impl Effect {
    fn infinite(&self) -> bool {
        self.duration == -1
    }

    /// `MobEffectInstance.compareTo`.
    pub fn compare(&self, o: &Effect) -> Ordering {
        if (self.duration <= 32147 || o.duration <= 32147) && !(self.ambient && o.ambient) {
            self.ambient
                .cmp(&o.ambient)
                .then(self.infinite().cmp(&o.infinite()))
                .then(self.duration.cmp(&o.duration))
                .then(self.color.cmp(&o.color))
        } else {
            self.ambient.cmp(&o.ambient).then(self.color.cmp(&o.color))
        }
    }

    /// `getEffectName`: the effect's name and its level from II to X.
    pub fn name(&self, lang: Lang) -> Text {
        let path = self.id.split_once(':').map_or(self.id.as_str(), |(_, p)| p);
        let name = Text::translate(&format!("effect.minecraft.{path}"), &[], lang);
        if (1..=9).contains(&self.amplifier) {
            name.append(" ").append(Text::translate(&format!("enchantment.level.{}", self.amplifier + 1), &[], lang))
        } else {
            name
        }
    }

    /// `MobEffectUtil.formatDuration(effect, 1, tickrate)`.
    pub fn duration_text(&self, tickrate: f32, lang: Lang) -> Text {
        if self.infinite() {
            return Text::translate("effect.duration.infinite", &[], lang);
        }
        let seconds = floor(floor(self.duration as f32) as f32 / tickrate);
        let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
        Text::literal(if hours > 0 { format!("{hours:02}:{minutes:02}:{seconds:02}") } else { format!("{minutes:02}:{seconds:02}") })
    }
}

/// `StringSplitter.headByWidth`: the longest start of `text` whose advances fit in `width`.
pub fn head_by_width(font: &dyn Metrics, text: &Text, width: f32) -> Text {
    let mut left = width;
    let mut n = 0;
    for (c, style) in text.resolve_legacy().chars() {
        left -= font.advance(c, style);
        if left < 0.0 {
            break;
        }
        n += 1;
    }
    text.resolve_legacy().slice(0, n)
}

/// `ComponentRenderUtils.clipText`: as much as fits in `width` with "..." after it.
pub fn clip(font: &dyn Metrics, text: &Text, width: i32) -> Text {
    let ellipsis = Text::literal("...");
    head_by_width(font, text, (width - font.width(&ellipsis)) as f32).append(ellipsis)
}

/// `EffectsInInventory.extractRenderState` beside `b`'s window; a clipped effect's tooltip goes
/// to `b.tooltip`.
pub fn extract(b: &mut Base, gui: &mut Gui, font: &dyn Metrics, lang: Lang, effects: &[Effect], mouse: [i32; 2], tickrate: f32) {
    let x0 = b.left + b.image_width + 2;
    let available = b.width - x0;
    if effects.is_empty() || available < 32 {
        return;
    }
    let max_width = if available >= 120 { available - 7 } else { 32 };
    let step = if effects.len() > 5 { 132 / (effects.len() as i32 - 1) } else { 33 };
    let mut sorted: Vec<&Effect> = effects.iter().collect();
    sorted.sort_by(|a, b| a.compare(b));
    let mut y0 = b.top;
    for e in sorted {
        let name = e.name(lang);
        let duration = e.duration_text(tickrate, lang);
        let width = max_width.min((32 + font.width(&name) + 7).max(32 + font.width(&duration) + 7));
        let background = if e.ambient { "container/inventory/effect_background_ambient" } else { "container/inventory/effect_background" };
        gui.sprite(background, x0, y0, width, 32);
        let text_width = width - 32 - 7;
        let compact = if text_width > 0 {
            let clipped = font.width(&name) > text_width;
            let shown = if clipped { clip(font, &name, text_width) } else { name.clone() };
            gui.text(shown, x0 + 32, y0 + 7, 0xFFFF_FFFF, true);
            gui.text(duration.clone(), x0 + 32, y0 + 16, 0xFF80_8080, true);
            clipped
        } else {
            true
        };
        if compact && mouse[0] >= x0 && mouse[0] <= x0 + width && mouse[1] >= y0 && mouse[1] <= y0 + step {
            let lines = vec![Line::Text(name), Line::Text(duration)];
            b.tooltip.set(Tooltip { lines, x: mouse[0], y: mouse[1], positioner: Positioner::Default, style: None, extra_space: false }, false);
        }
        let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
        gui.sprite(format!("mob_effect/{path}"), x0 + 7, y0 + 7, 18, 18);
        y0 += step;
    }
}
