//! `Fireworks.addToTooltip` and `FireworkExplosion.addToTooltip`: flight duration, runs of equal
//! explosions counted, each explosion's shape, colours by dye name, fade colours, trail and
//! twinkle. Ported from Horizon's `item_tooltip.rs` (MIT).

use serde_json::Value;

use super::fmt::{gray, indent, num};
use super::item::Details;
use super::stack::ItemStack;
use crate::java::text::Text;

/// `DyeColor` by its firework colour.
const DYE_FIREWORK_COLORS: [(&str, i64); 16] = [
    ("white", 15790320),
    ("orange", 15435844),
    ("magenta", 12801229),
    ("light_blue", 6719955),
    ("yellow", 14602026),
    ("lime", 4312372),
    ("pink", 14188952),
    ("gray", 4408131),
    ("light_gray", 11250603),
    ("cyan", 2651799),
    ("purple", 8073150),
    ("blue", 2437522),
    ("brown", 5320730),
    ("green", 3887386),
    ("red", 11743532),
    ("black", 1973019),
];

fn shape_name(e: &Value, d: &Details) -> Text {
    let shape = e.get("shape").and_then(Value::as_str).unwrap_or("small_ball");
    d.tr(&format!("item.minecraft.firework_star.shape.{shape}"), &[])
}

/// `appendColors`: dye names (else "Custom"), comma separated, after `prefix`.
fn colors(prefix: Text, colors: &[Value], d: &Details) -> Text {
    colors.iter().enumerate().fold(prefix, |t, (i, c)| {
        let c = c.as_i64().unwrap_or(0);
        let key = match DYE_FIREWORK_COLORS.iter().find(|(_, f)| *f == c) {
            Some((name, _)) => format!("item.minecraft.firework_star.{name}"),
            None => "item.minecraft.firework_star.custom_color".to_owned(),
        };
        let t = if i > 0 { t.append(", ") } else { t };
        t.append(d.tr(&key, &[]))
    })
}

/// `addAdditionalTooltip`.
fn additional(e: &Value, d: &Details, out: &mut Vec<Text>) {
    let list = |k: &str| e.get(k).and_then(Value::as_array).filter(|l| !l.is_empty());
    if let Some(c) = list("colors") {
        out.push(gray(colors(Text::default(), c, d)));
    }
    if let Some(c) = list("fade_colors") {
        out.push(gray(colors(d.tr("item.minecraft.firework_star.fade_to", &[]).append(" "), c, d)));
    }
    let flag = |k: &str| e.get(k).and_then(Value::as_bool).unwrap_or(false);
    if flag("has_trail") {
        out.push(gray(d.tr("item.minecraft.firework_star.trail", &[])));
    }
    if flag("has_twinkle") {
        out.push(gray(d.tr("item.minecraft.firework_star.flicker", &[])));
    }
}

pub fn explosion(_: &ItemStack, e: &Value, d: &Details, out: &mut Vec<Text>) {
    out.push(gray(shape_name(e, d)));
    additional(e, d, out);
}

pub fn fireworks(_: &ItemStack, v: &Value, d: &Details, out: &mut Vec<Text>) {
    let flight = v.get("flight_duration").and_then(Value::as_i64).unwrap_or(0);
    if flight > 0 {
        out.push(gray(d.tr("item.minecraft.firework_rocket.flight_duration", &[num(flight)])));
    }
    let explosions = v.get("explosions").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    for run in explosions.chunk_by(|a, b| a == b) {
        let e = &run[0];
        out.push(gray(match run.len() {
            1 => d.tr("item.minecraft.firework_rocket.single_star", &[shape_name(e, d)]),
            n => d.tr("item.minecraft.firework_rocket.multiple_stars", &[num(n), shape_name(e, d)]),
        }));
        let mut lines = Vec::new();
        additional(e, d, &mut lines);
        out.extend(lines.into_iter().map(|l| indent("  ", l)));
    }
}
