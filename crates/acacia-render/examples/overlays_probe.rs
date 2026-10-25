//! Probe of the parity harness overlays (tools/parity/harnesses/overlays): `overlay`'s texels,
//! creeper flash and shake. Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example overlays_probe -- <overlays.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::entity::java::mth::joml_rotation;
use acacia_render::entity::java::overlay::{self, Shake};
use glam::Vec3;
use serde_json::Value;

fn num(v: &Value) -> f32 {
    v.as_f64().unwrap_or(f64::NAN) as f32
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: overlays_probe <overlays.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let flag = |k: &str| r[k].as_bool().unwrap_or(false);
        match name.split('|').next() {
            Some("overlay") => {
                let hurt = flag("hurt");
                let u = overlay::u(num(&r["progress"]));
                case.numbers("u", &[num(&r["u"])], &[u as f32], 0.0);
                let texel = overlay::texel(u, hurt);
                if r["texel"].as_u64() != Some(u64::from(texel)) {
                    case.diverge("texel", r["texel"].to_string(), format!("{texel}"));
                }
            }
            Some("creeper") => {
                case.numbers("white", &[num(&r["white"])], &[overlay::creeper_white(num(&r["swelling"]))], 0.0);
            }
            Some("shake") => {
                let kind = r["kind"].as_str().unwrap_or_default();
                let shake = Shake { frozen: flag("frozen"), converting: flag("converting"), suffocating: flag("suffocating") };
                let shaking = overlay::shaking(kind, shake);
                if r["shaking"].as_bool() != Some(shaking) {
                    case.diverge("shaking", r["shaking"].to_string(), shaking);
                }
                let turn = if shaking { overlay::shake_degrees(num(&r["age"])) } else { 0.0 };
                let matrix = joml_rotation(Vec3::Y, 180.0 - turn);
                case.numbers("matrix", &compare::floats(&r["matrix"]), &matrix.to_cols_array(), 1e-6);
            }
            _ => case.diverge("case", "a known kind", name),
        }
    }
    out.print();
}
