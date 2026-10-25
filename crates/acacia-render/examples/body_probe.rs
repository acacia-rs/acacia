//! Probe of the parity harness body_transforms (tools/parity/harnesses/body_transforms):
//! `entity::java::body::transform` on each record's state against the oracle's matrix. Prints
//! `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example body_probe -- <body.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::entity::java::{RenderState, body};
use serde_json::Value;

fn main() {
    let path = std::env::args().nth(1).expect("usage: body_probe <body.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let mut case = out.case(r["case"].as_str().unwrap_or_default());
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let matrix = body::transform(r["kind"].as_str().unwrap_or_default(), &RenderState::from_json(&r["state"]));
        case.numbers("matrix", &compare::floats(&r["matrix"]), &matrix.to_cols_array(), 2e-5);
    }
    out.print();
}
