//! Probe of the parity harness name_tags (tools/parity/harnesses/name_tags): `types::name_tag`
//! per entity type, age and pose. Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example name_tags_probe -- <name_tags.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use acacia_render::entity::java::types;
use serde_json::Value;

fn main() {
    let path = std::env::args().nth(1).expect("usage: name_tags_probe <name_tags.jsonl>");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(path).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let mut parts = name.split('|');
        let (kind, age, pose) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let got = types::name_tag(kind, age == "baby", pose);
        match (r["at"].is_null(), got) {
            (true, None) => {}
            (false, Some(tag)) => {
                let at = [0.0, tag.height, 0.0];
                case.numbers("at", &compare::floats(&r["at"]), &at, 1e-5);
                case.numbers("turned", &compare::floats(&r["turned"]), &at, 1e-5);
                case.numbers("distance", &[r["distance"].as_f64().unwrap_or(0.0) as f32], &[tag.distance], 0.0);
            }
            (want_none, got) => case.diverge("tag", if want_none { "none" } else { "a tag" }, format!("{got:?}")),
        }
    }
    out.print();
}
