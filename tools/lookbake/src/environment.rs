//! The jar's environment for a Java look: the weather, cloud, sky and underwater textures and the
//! enchantment glints (`rain.png` and `snow.png`, which Bedrock folds into one `weather.png`;
//! Java's own `clouds.png` and sun, its moon phases as one 4×2 `moon_phases.png`, `end_sky.png`,
//! `underwater.png`, which Bedrock lacks), and `environment.json`: the timelines, dimension types
//! and biomes' rendered environment attributes (`acacia_render::sky::Environment`).

use std::collections::BTreeMap;
use std::path::Path;

use acacia_render::sky::timeline::MOON_PHASES;
use image::RgbaImage;
use serde_json::{Map, Value, json};

use crate::biomes::bedrock_names;
use crate::download::Error;
use crate::ui::copy_tree;

const COPIED: [(&str, &str); 8] = [
    ("textures/misc/enchanted_glint_item.png", "textures/misc/enchanted_glint_item.png"),
    ("textures/misc/enchanted_glint_armor.png", "textures/misc/enchanted_glint_armor.png"),
    ("textures/environment/rain.png", "textures/environment/rain.png"),
    ("textures/environment/snow.png", "textures/environment/snow.png"),
    ("textures/environment/clouds.png", "textures/environment/clouds.png"),
    ("textures/environment/end_sky.png", "textures/environment/end_sky.png"),
    ("textures/environment/celestial/sun.png", "textures/environment/sun.png"),
    ("textures/misc/underwater.png", "textures/misc/underwater.png"),
];

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    for (from, to) in COPIED {
        copy_tree(&assets.join(from), &out.join(to)).map_err(|e| format!("{}: {e}", assets.join(from).display()))?;
    }
    moon_phases(assets)?.save(out.join("textures/environment/moon_phases.png"))?;
    Ok(())
}

/// `celestial/moon/<phase>.png` on one sheet, four a row in `MoonPhase` order.
fn moon_phases(assets: &Path) -> Result<RgbaImage, Error> {
    let mut sheet: Option<RgbaImage> = None;
    for (i, phase) in MOON_PHASES.iter().enumerate() {
        let path = assets.join(format!("textures/environment/celestial/moon/{phase}.png"));
        let image = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.into_rgba8();
        let (w, h) = image.dimensions();
        let sheet = sheet.get_or_insert_with(|| RgbaImage::new(w * 4, h * 2));
        image::imageops::replace(sheet, &image, i64::from(i as u32 % 4 * w), i64::from(i as u32 / 4 * h));
    }
    Ok(sheet.expect("there are eight phases"))
}

/// What the renderer reads; the jar's other attributes (sounds, mob rules) are left out.
fn rendered(attribute: &str) -> bool {
    attribute.starts_with("minecraft:visual/") || attribute == "minecraft:gameplay/sky_light_level"
}

fn rendered_of(attributes: &Value) -> Map<String, Value> {
    attributes.as_object().into_iter().flatten().filter(|(k, _)| rendered(k)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Every `<name>.json` in `dir`, by name.
fn read_all(dir: &Path) -> Result<BTreeMap<String, Value>, Error> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry?.path();
        let (Some(name), Some("json")) = (path.file_stem().and_then(|n| n.to_str()), path.extension().and_then(|e| e.to_str())) else { continue };
        out.insert(name.to_owned(), serde_json::from_slice(&std::fs::read(&path)?)?);
    }
    Ok(out)
}

fn strip(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// A dimension's `timelines` (one id, a `#tag`, or a list of them) as timeline names, tags expanded.
fn timelines(entry: &Value, tags: &BTreeMap<String, Value>, out: &mut Vec<String>) {
    match entry {
        Value::Array(entries) => entries.iter().for_each(|e| timelines(e, tags, out)),
        Value::String(id) => match id.strip_prefix('#') {
            Some(tag) => timelines(&tags.get(strip(tag)).map_or(Value::Null, |t| t["values"].clone()), tags, out),
            None => out.push(strip(id).to_owned()),
        },
        _ => {}
    }
}

/// `environment.json` from the jar's `data/minecraft`.
pub fn write(data: &Path, out: &Path) -> Result<(), Error> {
    let timeline_docs = read_all(&data.join("timeline"))?;
    let tags = read_all(&data.join("tags/timeline"))?;
    let mut doc_timelines = Map::new();
    for (name, t) in &timeline_docs {
        let tracks = rendered_of(&t["tracks"]);
        if !tracks.is_empty() {
            doc_timelines.insert(name.clone(), json!({ "period_ticks": t["period_ticks"], "tracks": tracks }));
        }
    }
    let mut dimensions = Map::new();
    for (name, d) in read_all(&data.join("dimension_type"))? {
        let mut names = Vec::new();
        timelines(&d["timelines"], &tags, &mut names);
        names.retain(|n| doc_timelines.contains_key(n));
        let dimension = json!({
            "attributes": rendered_of(&d["attributes"]),
            "timelines": names,
            "has_skylight": d["has_skylight"],
            "has_ceiling": d["has_ceiling"],
            "skybox": d.get("skybox").cloned().unwrap_or_else(|| "overworld".into()),
        });
        dimensions.insert(name, dimension);
    }
    // A biome with no attributes at all shares Java's one empty map; any other is its own source.
    let mut biomes = Map::new();
    for (name, b) in read_all(&data.join("worldgen/biome"))? {
        if b["attributes"].as_object().is_some_and(|a| !a.is_empty()) {
            biomes.insert(name.clone(), json!({ "bedrock": bedrock_names(&name), "attributes": rendered_of(&b["attributes"]) }));
        }
    }
    let doc = json!({ "timelines": doc_timelines, "dimensions": dimensions, "biomes": biomes });
    std::fs::write(out.join("environment.json"), serde_json::to_vec_pretty(&doc)?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
