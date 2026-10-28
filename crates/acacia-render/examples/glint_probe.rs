//! Probe of the parity harness glint (tools/parity/harnesses/glint): `glint::texturing` and
//! `Foil::Java`. Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example glint_probe -- <glint.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::glint::{self, Foil};
use serde_json::Value;

fn main() {
    let path = std::env::args().nth(1).expect("usage: glint_probe <glint.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        if let Some(item) = r["item"].as_str() {
            let foil = Foil::Java.always(item);
            if r["foil"].as_bool() != Some(foil) {
                case.diverge("foil", r["foil"].to_string(), foil);
            }
            continue;
        }
        let scale = match r["transform"].as_str() {
            Some("GLINT_TEXTURING") => glint::ITEM_SCALE,
            Some("ENTITY_GLINT_TEXTURING") => glint::ENTITY_SCALE,
            _ => glint::ARMOR_SCALE,
        };
        let m = glint::texturing(r["millis"].as_u64().unwrap_or(0), r["speed"].as_f64().unwrap_or(0.0), scale);
        case.numbers("matrix", &compare::floats(&r["matrix"]), &m.to_cols_array(), 0.0);
    }
    out.print();
}
