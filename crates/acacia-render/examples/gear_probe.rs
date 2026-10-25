//! Probe of the parity harness entity_gear (tools/parity/harnesses/entity_gear):
//! `entity::java::gear::draws` on each record's gear against what the kind's render layers
//! submitted. Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example gear_probe -- <look dir> <gear.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::entity::Blend;
use acacia_render::entity::java::equipment::Equipment;
use acacia_render::entity::java::gear::{self, Body, Gear};
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [look, records] = args.as_slice() else { panic!("usage: gear_probe <look dir> <gear.jsonl>") };
    let equipment = Equipment::load(std::path::Path::new(look));
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(records).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let mut case = out.case(r["case"].as_str().unwrap_or_default());
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let kind = r["kind"].as_str().unwrap_or_default();
        let g = &r["gear"];
        let item = g["body"].as_str().unwrap_or_default();
        let max = g["maxDamage"].as_i64().unwrap_or(0);
        let durability = (max > 0).then(|| (max - g["damage"].as_i64().unwrap_or(0)) as f32 / max as f32);
        let body = (!item.is_empty()).then(|| Body { item, dye: g["dye"].as_i64().map(|c| rgb(c as u32)), durability });
        let gear = Gear {
            baby: g["baby"].as_bool().unwrap_or(false),
            invisible: g["invisible"].as_bool().unwrap_or(false),
            saddled: g["saddled"].as_bool().unwrap_or(false),
            body,
            collar: g["collar"].as_i64().filter(|c| *c >= 0).map(|c| c as usize),
            markings: g["markings"].as_u64().unwrap_or(0) as usize,
            leash_holder: g["leashHolder"].as_bool().unwrap_or(false),
        };
        let body_layer = format!("{kind}{}#main", if gear.baby { "_baby" } else { "" });
        let got = gear::draws(kind, &gear, &equipment);
        let expected = r["draws"].as_array().cloned().unwrap_or_default();
        let got_text: Vec<String> = got.iter().map(|d| format!("{} {} {:?} {:?}", d.layer.unwrap_or(&body_layer), d.texture, d.color, d.blend)).collect();
        if expected.len() != got.len() {
            let want: Vec<String> = expected.iter().map(|d| format!("{} {} {}", d["layers"], d["texture"], d["renderType"])).collect();
            case.diverge("draws", &want.join(" | "), &got_text.join(" | "));
            continue;
        }
        for (i, (want, got)) in expected.iter().zip(&got).enumerate() {
            let layers: Vec<&str> = want["layers"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let layer = got.layer.unwrap_or(&body_layer);
            if !layers.contains(&layer) {
                case.diverge(&format!("draw {i} layer"), &format!("{layers:?}"), layer);
            }
            let texture = want["texture"].as_str().unwrap_or_default().replace("minecraft:", "java/");
            if texture != got.texture {
                case.diverge(&format!("draw {i} texture"), &texture, &got.texture);
            }
            let color = want["color"].as_i64().unwrap_or(-1) as u32;
            let color = (color != u32::MAX).then(|| rgb(color));
            if color != got.color {
                case.diverge(&format!("draw {i} color"), &format!("{color:?}"), &format!("{:?}", got.color));
            }
            let blend = blend(want["renderType"].as_str().unwrap_or_default());
            if blend != Some(got.blend) {
                case.diverge(&format!("draw {i} blend"), &want["renderType"], &format!("{:?}", got.blend));
            }
        }
    }
    out.print();
}

fn rgb(c: u32) -> [u8; 3] {
    let [_, r, g, b] = c.to_be_bytes();
    [r, g, b]
}

/// The blend acacia draws a render type with.
fn blend(render_type: &str) -> Option<Blend> {
    match render_type {
        "armor_cutout_no_cull" | "entity_cutout" | "entity_cutout_no_cull" => Some(Blend::Opaque),
        "entity_translucent" | "wolf_armor_cracks" => Some(Blend::Alpha),
        _ => None,
    }
}
