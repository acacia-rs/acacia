//! `cargo run -p acacia-codegen`: regenerates crates/acacia-proto/src/generated from the vendored minecraft-data.

mod emit_items;
mod emit_read;
mod emit_write;
mod ir;
mod lower;
mod lower_switch;
mod names;
mod output;
mod schema;

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

fn read_json(path: &Path) -> Value {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Replaces or adds `types` entries from overrides.json (local corrections to minecraft-data).
fn apply_overrides(json: &mut Value, overrides: &Value) {
    let types = json["types"].as_object_mut().expect("protocol.json types");
    for (name, def) in overrides["types"].as_object().expect("overrides.json types") {
        types.insert(name.clone(), def.clone());
    }
}

/// Maps packet type name (without `packet_`) to (id, wire name) using the `mcpe_packet` container.
fn packet_ids(json: &Value) -> HashMap<String, (u32, String)> {
    let fields = json["types"]["mcpe_packet"][1]
        .as_array()
        .expect("mcpe_packet container");
    let mappings = fields[0]["type"][1]["mappings"]
        .as_object()
        .expect("packet id mapper");
    let params = fields[1]["type"][1]["fields"]
        .as_object()
        .expect("packet params switch");
    let mut out = HashMap::new();
    for (id, name) in mappings {
        let name = name.as_str().unwrap();
        let ty = params
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("no params for packet {name}"));
        let ty = ty.strip_prefix("packet_").expect("packet params type");
        out.insert(
            ty.to_owned(),
            (id.parse().expect("packet id"), name.to_owned()),
        );
    }
    out
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("data");
    let out = root.join("../../crates/acacia-proto/src/generated");

    let mut json = read_json(&data.join("protocol.json"));
    apply_overrides(&mut json, &read_json(&data.join("overrides.json")));
    let version = read_json(&data.join("version.json"));
    let version = output::Version {
        protocol: version["version"].as_i64().expect("version.version"),
        game: version["minecraftVersion"]
            .as_str()
            .expect("version.minecraftVersion")
            .to_owned(),
    };

    let schema = schema::Schema::parse(&json);
    let ids = packet_ids(&json);
    let lowered = lower::lower(&schema, ids);
    let packets = lowered.items.iter().filter(|i| i.packet.is_some()).count();
    let files = output::write_all(&out, &lowered.items, &version);
    output::rustfmt(&files);
    println!(
        "generated {} items ({packets} packets) into {} files",
        lowered.items.len(),
        files.len()
    );
}
