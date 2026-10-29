//! Probe of the parity harness armor_trims (tools/parity/harnesses/armor_trims): per equipment
//! asset, layer type, pattern and material, the image `Equipment::trim_texture` picks against the
//! texture the game's `TrimTextureKey` asks for, and that image's pixels (as lookbake baked it)
//! against the game's, by FNV-1a 64 of the RGBA bytes.
//! Prints `CASES\t<n>` and `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>`.
//!
//! cargo run -p acacia-render --example armor_trims -- <look dir> <trims.jsonl>

#[path = "java_entities/compare.rs"]
mod compare;

use std::path::Path;

use acacia_render::entity::java::equipment::Equipment;
use acacia_render::entity::java::trims::{Trim, Trims, baked_path};
use serde_json::Value;

fn fnv(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:x}")
}

/// The path the game's ask maps to in the look.
fn expected(layer_type: &str, base: &str, palette: Option<&str>) -> String {
    match palette {
        Some(palette) => {
            let (namespace, path) = base.split_once(':').unwrap_or(("minecraft", base));
            let asset = path.trim_start_matches(&format!("trims/entity/{layer_type}/"));
            baked_path(layer_type, &format!("{namespace}:{asset}"), palette)
        }
        None => format!("java/{}", base.split_once(':').map_or(base, |(_, p)| p)),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (root, records) = (args.next().expect("look dir"), args.next().expect("trims.jsonl"));
    let root = Path::new(&root);
    let (equipment, trims) = (Equipment::load(root), Trims::load(root));
    let mut out = compare::Report::default();
    for line in std::fs::read_to_string(records).expect("oracle records").lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).expect("oracle record");
        let name = r["case"].as_str().unwrap_or_default();
        let mut case = out.case(name);
        if let Some(error) = r.get("error") {
            case.diverge("oracle", "a texture", error);
            continue;
        }
        let [asset, layer_type, pattern, material] = name.splitn(4, '|').collect::<Vec<_>>()[..] else { continue };
        let want = expected(layer_type, r["base"].as_str().unwrap_or_default(), r["palette"].as_str());
        let Some(got) = equipment.trim_texture(asset, layer_type, &trims, &Trim { material: material.into(), pattern: pattern.into() }) else {
            case.diverge("texture", &want, "none");
            continue;
        };
        if got != want {
            case.diverge("texture", &want, &got);
            continue;
        }
        match image::open(root.join(&got)) {
            Ok(image) => {
                let hash = fnv(image.into_rgba8().as_raw());
                if r["hash"].as_str() != Some(hash.as_str()) {
                    case.diverge("pixels", &r["hash"], &hash);
                }
            }
            Err(e) => case.diverge("pixels", &r["hash"], format!("{got}: {e}")),
        }
    }
    out.print();
}
