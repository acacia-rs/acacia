//! Probe of the entity parity harnesses (tools/parity/harnesses/entity_models, entity_poses,
//! entity_layers): draws the look's Java entity models as the entity pass does, CPU-side, and
//! compares them with the oracle's records. Prints `CASES\t<n>` and
//! `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example java_entities -- models|poses|layers <entity_models.json> <records.jsonl>

mod compare;
mod drawn;
mod layers;
mod poses;

use std::collections::BTreeMap;

use acacia_render::entity::java::{Part, model::parse_layers};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [mode, models, records] = args.as_slice() else {
        eprintln!("usage: java_entities models|poses|layers <entity_models.json> <records.jsonl>");
        std::process::exit(2);
    };
    let layers: BTreeMap<String, Part> = parse_layers(&std::fs::read_to_string(models).expect("entity models")).expect("entity models json");
    let records = std::fs::read_to_string(records).expect("oracle records");
    let records = records.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).expect("oracle record"));
    let mut out = compare::Report::default();
    match mode.as_str() {
        "models" => records.for_each(|r| drawn::rest(&layers, &r, &mut out)),
        "poses" => records.for_each(|r| poses::posed(&layers, &r, &mut out)),
        "layers" => records.for_each(|r| layers::layers(&layers, &r, &mut out)),
        _ => panic!("unknown mode {mode}"),
    }
    out.print();
}
