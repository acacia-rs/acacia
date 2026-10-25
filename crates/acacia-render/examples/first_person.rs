//! Probe of the parity harness item_transforms (tools/parity/harnesses/item_transforms): our
//! first-person item placement (item::first_person::frame) against the oracle's matrix per case.
//! Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example first_person -- <hands.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::item::first_person::{Hold, Using, frame};
use serde_json::Value;

fn using(v: &Value) -> Option<Using> {
    let held = v["held"].as_f64()? as f32;
    Some(match v["kind"].as_str()? {
        "eat" => Using::Eat { held, duration: v["duration"].as_f64()? as f32 },
        "block" => Using::Block,
        "bow" => Using::Bow { held },
        "trident" => Using::Trident { held },
        "crossbow" => Using::Crossbow { held, charge: v["charge"].as_f64()? as f32 },
        _ => return None,
    })
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: first_person <hands.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let mut case = out.case(r["case"].as_str().unwrap_or_default());
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a matrix", error);
            continue;
        }
        let flag = |k: &str| r[k].as_bool().unwrap_or(false);
        let num = |k: &str| r[k].as_f64().unwrap_or(0.0) as f32;
        let hold = Hold {
            left: flag("left"),
            main_hand: flag("main_hand"),
            lowered: num("lowered"),
            attack: num("attack"),
            swinging: flag("swinging"),
            using: using(&r["using"]),
            crossbow: r["crossbow"].as_bool(),
            spinning: flag("spinning"),
        };
        case.numbers("matrix", &compare::floats(&r["matrix"]), &frame(&hold).to_cols_array(), 1e-5);
    }
    out.print();
}
