//! `PotionContents.addPotionTooltip` and the components that use it: `potion_contents` (the
//! potion's effects then the custom ones, durations scaled by `potion_duration_scale`),
//! `ominous_bottle_amplifier` (Bad Omen) and, in creative, `suspicious_stew_effects`.

use serde_json::Value;

use super::attributes::{Modifier, signed_line};
use super::fmt::{BLUE, DARK_PURPLE, GRAY, RED, formatting};
use super::item::Details;
use super::stack::ItemStack;
use crate::java::text::Text;

/// A `MobEffectInstance`: effect id, duration in ticks (-1 infinite), amplifier.
pub struct Effect<'a> {
    pub id: &'a str,
    pub duration: i32,
    pub amplifier: i32,
}

fn effect(v: &Value) -> Option<Effect<'_>> {
    let int = |k: &str, d: i64| v.get(k).and_then(Value::as_i64).unwrap_or(d) as i32;
    Some(Effect { id: v.get("id")?.as_str()?, duration: int("duration", 0), amplifier: int("amplifier", 0) })
}

/// `StringUtil.formatTickDuration`.
fn tick_duration(ticks: i32, tick_rate: f32) -> String {
    let seconds = (ticks as f32 / tick_rate).floor() as i32;
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let (hours, minutes) = (minutes / 60, minutes % 60);
    if hours > 0 { format!("{hours:02}:{minutes:02}:{seconds:02}") } else { format!("{minutes:02}:{seconds:02}") }
}

/// `MobEffectUtil.formatDuration`.
fn duration(e: &Effect, scale: f32, tick_rate: f32, d: &Details) -> Text {
    if e.duration == -1 {
        return d.tr("effect.duration.infinite", &[]);
    }
    Text::literal(tick_duration((e.duration as f32 * scale).floor() as i32, tick_rate))
}

/// `addPotionTooltip(effects, lines, durationScale, tickrate)`.
pub fn potion_lines(effects: &[Effect], scale: f32, d: &Details, out: &mut Vec<Text>) {
    let tick_rate = d.game.0.get("tickRate").and_then(Value::as_f64).unwrap_or(20.0) as f32;
    let mut modifiers = Vec::new();
    for e in effects {
        let facts = d.game.entry("mob_effect", e.id);
        for m in facts.and_then(|f| f.get("modifiers")).and_then(Value::as_array).into_iter().flatten() {
            let (Some(attribute), Some(amount), Some(operation)) =
                (m.get("attribute").and_then(Value::as_str), m.get("amount").and_then(Value::as_f64), m.get("operation").and_then(Value::as_str))
            else {
                continue;
            };
            modifiers.push(Modifier { attribute, id: "", amount: amount * f64::from(e.amplifier + 1), operation });
        }
        let path = e.id.strip_prefix("minecraft:").unwrap_or(e.id);
        let key = facts.and_then(|f| f.get("description")).and_then(Value::as_str).map_or_else(|| format!("effect.minecraft.{path}"), str::to_owned);
        let mut line = d.tr(&key, &[]);
        if e.amplifier > 0 {
            line = d.tr("potion.withAmplifier", &[line, d.tr(&format!("potion.potency.{}", e.amplifier), &[])]);
        }
        if e.duration == -1 || e.duration > 20 {
            line = d.tr("potion.withDuration", &[line, duration(e, scale, tick_rate, d)]);
        }
        let category = facts.and_then(|f| f.get("category")).and_then(Value::as_str).unwrap_or("blue");
        out.push(line.with_style(&formatting(category)));
    }
    if effects.is_empty() {
        out.push(d.tr("effect.none", &[]).with_color(GRAY));
    }
    if modifiers.is_empty() {
        return;
    }
    out.push(Text::default());
    out.push(d.tr("potion.whenDrank", &[]).with_color(DARK_PURPLE));
    for m in &modifiers {
        let shown = if m.multiplied() { m.amount * 100.0 } else { m.amount };
        if let Some((line, increase)) = signed_line(m, shown, d) {
            out.push(line.with_color(if increase { BLUE } else { RED }));
        }
    }
}

/// `PotionContents.getAllEffects`: the potion's, then the custom ones.
pub fn all_effects<'a>(contents: &'a Value, d: &'a Details) -> Vec<Effect<'a>> {
    let potion = contents.get("potion").and_then(Value::as_str).and_then(|p| d.game.entry("potion", p));
    let base = potion.and_then(|p| p.get("effects")).and_then(Value::as_array).into_iter().flatten();
    let custom = contents.get("custom_effects").and_then(Value::as_array).into_iter().flatten();
    base.chain(custom).filter_map(effect).collect()
}

pub fn potion_contents(stack: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let scale = stack.get("potion_duration_scale").and_then(Value::as_f64).unwrap_or(1.0) as f32;
    potion_lines(&all_effects(v, d), scale, d, out);
}

pub fn ominous_bottle(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let amplifier = v.as_i64().unwrap_or(0) as i32;
    potion_lines(&[Effect { id: "minecraft:bad_omen", duration: 120000, amplifier }], 1.0, d, out);
}

pub fn suspicious_stew(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    if !d.creative {
        return;
    }
    let effects: Vec<Effect> = v.as_array().into_iter().flatten().filter_map(|e| {
        Some(Effect { id: e.get("id")?.as_str()?, duration: e.get("duration").and_then(Value::as_i64).unwrap_or(160) as i32, amplifier: 0 })
    }).collect();
    potion_lines(&effects, 1.0, d, out);
}
