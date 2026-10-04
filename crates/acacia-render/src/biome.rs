//! Biome tint colours. Grass and foliage come from the vanilla colormaps indexed by the server's
//! temperature and downfall (`BiomeDefinitionList`), with vanilla's hard-coded exceptions; water
//! comes from `biomes_client.json`.

use std::path::Path;

use rustc_hash::FxHashMap;
use serde_json::Value;

use crate::assets::json;

/// One entry of the server's `BiomeDefinitionList`.
#[derive(Debug, Clone)]
pub struct BiomeDef {
    pub id: u16,
    /// Without namespace, e.g. `plains`.
    pub name: String,
    pub temperature: f32,
    pub downfall: f32,
}

/// sRGB colours a biome gives tinted blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiomeTint {
    pub grass: [u8; 3],
    pub foliage: [u8; 3],
    pub water: [u8; 3],
}

const PLAINS: BiomeTint = BiomeTint { grass: rgb(0x91BD59), foliage: rgb(0x77AB2F), water: rgb(0x44AFF5) };

pub struct BiomeColors {
    by_id: FxHashMap<u32, BiomeTint>,
}

impl Default for BiomeColors {
    /// Plains everywhere, until the server's definitions arrive.
    fn default() -> Self {
        BiomeColors { by_id: FxHashMap::default() }
    }
}

impl BiomeColors {
    /// `root` holds the colormaps and `biomes_client.json`: a resource pack or a look pack's files.
    pub fn build(defs: &[BiomeDef], root: &Path) -> BiomeColors {
        let grass = Colormap::load(root, "grass");
        let foliage = Colormap::load(root, "foliage");
        let water = water_colors(root);
        let by_id = defs
            .iter()
            .map(|d| {
                let (g, f) = (grass.sample(d), foliage.sample(d));
                let (grass, foliage) = exceptions(&d.name, g, f);
                let water = water.get(d.name.as_str()).copied().unwrap_or(PLAINS.water);
                (u32::from(d.id), BiomeTint { grass, foliage, water })
            })
            .collect();
        BiomeColors { by_id }
    }

    /// Unknown ids (and chunks without biomes) tint like plains.
    pub fn get(&self, id: u32) -> &BiomeTint {
        self.by_id.get(&id).unwrap_or(&PLAINS)
    }
}

/// Vanilla's per-biome overrides of the colormap.
fn exceptions(name: &str, grass: [u8; 3], foliage: [u8; 3]) -> ([u8; 3], [u8; 3]) {
    match name {
        n if n.starts_with("swamp") => (rgb(0x6A7039), rgb(0x6A7039)),
        "mangrove_swamp" => (rgb(0x6A7039), rgb(0x8DB127)),
        n if n.starts_with("mesa") || n.contains("badlands") => (rgb(0x90814D), rgb(0x9E814D)),
        "cherry_grove" => (rgb(0xB6DB61), rgb(0xB6DB61)),
        "pale_garden" => (rgb(0x778272), rgb(0x878D76)),
        // Dark forest averages the colormap with a dark green.
        n if n.starts_with("roofed_forest") => (grass.map2(rgb(0x28340A), |a, b| ((u16::from(a) + u16::from(b)) / 2) as u8), foliage),
        _ => (grass, foliage),
    }
}

trait Map2 {
    fn map2(self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self;
}

impl Map2 for [u8; 3] {
    fn map2(self, other: Self, f: impl Fn(u8, u8) -> u8) -> Self {
        [f(self[0], other[0]), f(self[1], other[1]), f(self[2], other[2])]
    }
}

const fn rgb(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// A 256×256 colormap, indexed by temperature (x) and temperature-scaled downfall (y).
struct Colormap {
    image: Option<image::RgbImage>,
}

impl Colormap {
    fn load(root: &Path, name: &str) -> Colormap {
        let path = root.join(format!("textures/colormap/{name}.png"));
        let image = image::open(&path).inspect_err(|e| tracing::warn!(%e, path = %path.display(), "colormap")).ok();
        Colormap { image: image.map(|i| i.into_rgb8()) }
    }

    fn sample(&self, d: &BiomeDef) -> [u8; 3] {
        let Some(image) = &self.image else { return PLAINS.grass };
        let t = d.temperature.clamp(0.0, 1.0);
        let rain = d.downfall.clamp(0.0, 1.0) * t;
        let (w, h) = image.dimensions();
        let x = (((1.0 - t) * (w - 1) as f32) as u32).min(w - 1);
        let y = (((1.0 - rain) * (h - 1) as f32) as u32).min(h - 1);
        image.get_pixel(x, y).0
    }
}

/// `water_surface_color` by biome name (namespace stripped).
fn water_colors(root: &Path) -> FxHashMap<String, [u8; 3]> {
    let Ok(doc) = json::read(&root.join("biomes_client.json")) else { return FxHashMap::default() };
    let Some(biomes) = doc.get("biomes").and_then(Value::as_object) else { return FxHashMap::default() };
    biomes
        .iter()
        .filter_map(|(name, b)| {
            let hex = b.get("water_surface_color")?.as_str()?.strip_prefix('#')?;
            let short = name.strip_prefix("minecraft:").unwrap_or(name);
            Some((short.to_owned(), rgb(u32::from_str_radix(hex, 16).ok()?)))
        })
        .collect()
}
