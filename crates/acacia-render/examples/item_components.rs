//! Probe of the parity harness item_models' stack components: the tints a potion-holding item's
//! model gives for each potion, against the look's default tints with `item::potion::color` in
//! the potion's slot; a trimmed item's images, against the model `JavaItems::stack_name` picks.
//! Prints `CASES\t<n>` and `DIVERGENCE\t...`.
//!
//! cargo run -p acacia-render --example item_components -- <look dir> <item_components.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use std::path::Path;

use acacia_render::item::JavaItems;
use acacia_render::item::java::context;
use acacia_render::item::potion;
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [look, records] = args.as_slice() else {
        eprintln!("usage: item_components <look dir> <item_components.jsonl>");
        std::process::exit(2);
    };
    let items = JavaItems::load(Path::new(look)).expect("the look's java/item_models.json");
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(records).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        if name.starts_with("trim|") {
            trim(&items, &r, &mut case);
            continue;
        }
        let (item, id) = (r["item"].as_str().unwrap_or_default(), r["potion"].as_str().unwrap_or_default());
        let Some(layers) = items.layers(item, context::GUI) else {
            case.diverge("layers", "the item's", "none");
            continue;
        };
        let tint = potion::color(id);
        let expected = r["tints"].as_array().map_or(&[][..], Vec::as_slice);
        if expected.len() != layers.len() {
            case.diverge("layers.len", expected.len(), layers.len());
            continue;
        }
        for (i, (want, layer)) in expected.iter().zip(layers).enumerate() {
            let got: Vec<f32> = layer.tints.iter().enumerate().map(|(t, &argb)| if t == 0 { tint as i32 } else { argb } as f32).collect();
            let want: Vec<f32> = want.as_array().into_iter().flatten().map(|v| v.as_i64().unwrap_or(0) as i32 as f32).collect();
            case.numbers(&format!("tints[{i}]"), &want, &got, 0.0);
        }
    }
    out.print();
}

/// A trimmed stack's GUI layers, as `JavaItems::stack_name` picks them for Bedrock's `Trim.Material`:
/// per layer the images its quads sample, in order of first use.
fn trim(items: &JavaItems, r: &Value, case: &mut compare::Case) {
    let (item, material) = (r["item"].as_str().unwrap_or_default(), r["material"].as_str().unwrap_or_default());
    let name = items.stack_name(item, 0, Some(material));
    let Some(layers) = items.layers(&name, context::GUI) else {
        return case.diverge("layers", "the trimmed item's", "none");
    };
    let got: Vec<Vec<&str>> = layers.iter().map(|layer| {
        let mut images: Vec<&str> = Vec::new();
        for quad in items.quads(layer) {
            if !images.contains(&quad.image.as_str()) {
                images.push(&quad.image);
            }
        }
        images
    }).collect();
    let want: Vec<Vec<&str>> = r["images"].as_array().into_iter().flatten()
        .map(|layer| layer.as_array().into_iter().flatten().filter_map(Value::as_str).collect())
        .collect();
    if want != got {
        case.diverge("images", format!("{want:?}"), format!("{got:?}"));
    }
}
