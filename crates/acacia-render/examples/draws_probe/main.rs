//! Probe of the parity harness entity_draws (tools/parity/harnesses/entity_draws): the layers
//! `Library::appearance` and `Library::gear` give each record's kind, its state's fields answered
//! as Bedrock would ([`bridge`]), against what the kind's renderer submitted: per texture, the
//! model layer, colour and blend. A state no Bedrock server can send is counted, not compared.
//! Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example draws_probe -- <look dir> <draws.jsonl>

#[path = "../java_entities/compare.rs"]
mod compare;
mod bridge;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use acacia_render::entity::java::library::Library;
use acacia_render::entity::java::{Part, model};
use acacia_render::entity::{Blend, Layer};
use serde_json::Value;

/// One draw, comparable across the two sides: model layer names, texture, colour, and acacia's
/// blend or the render type Java submitted.
#[derive(Debug, Clone)]
struct Drawn {
    layers: Vec<String>,
    texture: String,
    color: Option<[u8; 3]>,
    /// Java's `ARGB.white(alpha)` channel.
    alpha: u8,
    blend: String,
}

/// EnergySwirlLayer's grey, which acacia's swirl shader applies itself.
const SWIRL_GREY: [u8; 3] = [128; 3];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [look, records] = args.as_slice() else { panic!("usage: draws_probe <look dir> <draws.jsonl>") };
    let root = Path::new(look);
    let text = std::fs::read_to_string(root.join("java/entity_models.json")).expect("entity models");
    let layers = model::parse_layers(&text).expect("entity models");
    let (mut meshes, mut textures) = (Vec::<Option<Arc<Part>>>::new(), Vec::<String>::new());
    let library = Library::load(
        root,
        &mut |mesh| {
            meshes.push(mesh.java.clone());
            (meshes.len() - 1) as u32
        },
        &mut |path| {
            textures.push(path.to_owned());
            Some((textures.len() - 1) as u32)
        },
    );
    let names = |id: u32| -> Vec<String> {
        let Some(Some(part)) = meshes.get(id as usize) else { return Vec::new() };
        layers.iter().filter(|(_, p)| *p == part.as_ref()).map(|(n, _)| n.clone()).collect()
    };
    let ours = |layer: &Layer, age: f32| Drawn {
        layers: names(layer.model),
        texture: textures.get(layer.textures[0] as usize).map_or_else(String::new, |t| t.trim_end_matches(".png").to_owned()),
        color: if layer.blend == Blend::Swirl { layer.dye.or(Some(SWIRL_GREY)) } else { layer.dye },
        alpha: (layer.fade.alpha(age) * 255.0).floor() as u8,
        blend: format!("{:?}", layer.blend),
    };
    let mut out = compare::Report::default();
    let mut unreachable = 0;
    for line in std::fs::read_to_string(records).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let mut case = out.case(r["case"].as_str().unwrap_or_default());
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a record", error);
            continue;
        }
        let kind = r["kind"].as_str().unwrap_or_default();
        if bridge::unreachable(kind, &r["fields"]).is_some() {
            unreachable += 1;
            continue;
        }
        let query = |name: &str| bridge::query(kind, &r["fields"], name);
        let bedrock = bridge::bedrock_kind(kind);
        let Some((body, _)) = library.appearance(&bedrock, &query) else {
            case.diverge("kind", "drawn by a Java model", "not");
            continue;
        };
        let age = r["fields"]["ageInTicks"].as_f64().unwrap_or(0.0) as f32;
        let all = body.iter().chain(&library.gear(&bedrock, &query, None)).cloned().collect::<Vec<_>>();
        let got: Vec<Drawn> = all.iter().filter(|l| l.fade.alpha(age) > 1e-5).map(|l| ours(l, age)).collect();
        let expected: Vec<Drawn> = r["draws"].as_array().into_iter().flatten().map(theirs).collect();
        compare(&mut case, expected, got);
    }
    eprintln!("{unreachable} states no Bedrock server sends, not compared");
    out.print();
}

fn theirs(d: &Value) -> Drawn {
    let color = d["color"].as_i64().unwrap_or(-1) as u32;
    let [_, r, g, b] = color.to_be_bytes();
    Drawn {
        layers: d["layers"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect(),
        texture: d["texture"].as_str().unwrap_or_default().replace("minecraft:", "java/").trim_end_matches(".png").to_owned(),
        color: (color & 0xFF_FFFF != 0xFF_FFFF).then_some([r, g, b]),
        alpha: (color >> 24) as u8,
        blend: d["renderType"].as_str().unwrap_or_default().to_owned(),
    }
}

/// The blends acacia may draw a Java render type with: `entity_translucent` is a whole body
/// writing depth or a layer over one leaving it.
fn blends(render_type: &str) -> &'static [Blend] {
    match render_type {
        "entity_cutout" | "entity_cutout_no_cull" | "entity_cutout_cull" | "entity_solid" | "armor_cutout_no_cull" | "entity_cutout_z_offset" => &[Blend::Opaque],
        "entity_translucent" => &[Blend::Translucent, Blend::Alpha],
        "entity_translucent_cull" | "item_entity_translucent_cull" | "wolf_armor_cracks" => &[Blend::Alpha],
        "eyes" | "entity_translucent_emissive" => &[Blend::Eyes],
        "energy_swirl" => &[Blend::Swirl],
        "breeze_wind" => &[Blend::Wind],
        _ => &[],
    }
}

/// Pairs the draws by texture (each side in order) and compares the rest; a texture on one side
/// only is a divergence.
fn compare(case: &mut compare::Case<'_>, expected: Vec<Drawn>, got: Vec<Drawn>) {
    let mut by_texture: BTreeMap<String, (Vec<Drawn>, Vec<Drawn>)> = BTreeMap::new();
    for d in expected {
        by_texture.entry(d.texture.clone()).or_default().0.push(d);
    }
    for d in got {
        by_texture.entry(d.texture.clone()).or_default().1.push(d);
    }
    for (texture, (want, have)) in by_texture {
        if want.len() != have.len() {
            let layers = |v: &[Drawn]| v.iter().map(|d| d.layers.first().cloned().unwrap_or_default()).collect::<Vec<_>>().join(",");
            case.diverge(&format!("draws of {texture}"), format!("{} [{}]", want.len(), layers(&want)), format!("{} [{}]", have.len(), layers(&have)));
            continue;
        }
        for (w, h) in want.iter().zip(&have) {
            if !h.layers.iter().any(|l| w.layers.contains(l)) {
                case.diverge(&format!("{texture} layer"), format!("{:?}", w.layers), format!("{:?}", h.layers));
            }
            if w.color != h.color {
                case.diverge(&format!("{texture} color"), format!("{:?}", w.color), format!("{:?}", h.color));
            }
            if w.alpha != h.alpha {
                case.diverge(&format!("{texture} alpha"), w.alpha, h.alpha);
            }
            if !blends(&w.blend).iter().any(|b| format!("{b:?}") == h.blend) {
                case.diverge(&format!("{texture} blend"), &w.blend, &h.blend);
            }
        }
    }
}
