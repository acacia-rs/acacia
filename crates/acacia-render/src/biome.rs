//! Biome tint colours. Grass and foliage come from the vanilla colormaps indexed by the server's
//! temperature and downfall (`BiomeDefinitionList`), with vanilla's hard-coded exceptions; water
//! comes from `biomes_client.json`.

pub mod ids;
pub mod noise;

use std::path::Path;

use rustc_hash::FxHashMap;
use serde_json::Value;

use crate::assets::json;

/// One entry of the server's `BiomeDefinitionList`.
#[derive(Debug, Clone)]
pub struct BiomeDef {
    /// [`ids::UNSET`] for a vanilla biome.
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
    /// Grass where [`noise::grass_patch`] says, as in Java's swamps.
    pub grass_patch: Option<[u8; 3]>,
    pub foliage: [u8; 3],
    /// Leaf litter.
    pub dry_foliage: [u8; 3],
    pub water: [u8; 3],
}

impl BiomeTint {
    /// Grass in the column of world position `(x, z)`.
    pub fn grass_at(&self, x: i32, z: i32) -> [u8; 3] {
        self.grass_patch.filter(|_| noise::grass_patch(x, z)).unwrap_or(self.grass)
    }
}

pub(crate) const PLAINS: BiomeTint =
    BiomeTint { grass: rgb(0x91BD59), grass_patch: None, foliage: rgb(0x77AB2F), dry_foliage: rgb(0xA37546), water: rgb(0x44AFF5) };

/// What `biomes_client.json` says of a biome. Bedrock's has the water; a look pack's can list the
/// rest, which then replaces the colormaps.
#[derive(Default, Clone, Copy)]
struct Listed {
    water: Option<[u8; 3]>,
    grass: Option<[u8; 3]>,
    grass_patch: Option<[u8; 3]>,
    foliage: Option<[u8; 3]>,
    dry_foliage: Option<[u8; 3]>,
}

pub struct BiomeColors {
    by_id: FxHashMap<u32, BiomeTint>,
    unknown: BiomeTint,
}

impl Default for BiomeColors {
    /// Plains everywhere, until the server's definitions arrive.
    fn default() -> Self {
        BiomeColors { by_id: FxHashMap::default(), unknown: PLAINS }
    }
}

impl BiomeColors {
    /// `root` holds the colormaps and `biomes_client.json`: a resource pack or a look pack's files.
    pub fn build(defs: &[BiomeDef], root: &Path) -> BiomeColors {
        let grass = Colormap::load(root, "grass");
        let foliage = Colormap::load(root, "foliage");
        let listed = listed(root);
        let unknown = BiomeTint { water: listed.get("default").and_then(|l| l.water).unwrap_or(PLAINS.water), ..PLAINS };
        let by_id = defs
            .iter()
            .filter_map(|d| {
                let id = if d.id == ids::UNSET { ids::vanilla(&d.name)? } else { d.id };
                let (g, f) = (grass.sample(d), foliage.sample(d));
                let (g, f) = exceptions(&d.name, g, f);
                let l = listed.get(d.name.as_str()).copied().unwrap_or_default();
                let tint = BiomeTint {
                    grass: l.grass.unwrap_or(g),
                    grass_patch: l.grass_patch,
                    foliage: l.foliage.unwrap_or(f),
                    dry_foliage: l.dry_foliage.unwrap_or(unknown.dry_foliage),
                    water: l.water.unwrap_or(unknown.water),
                };
                Some((u32::from(id), tint))
            })
            .collect();
        BiomeColors { by_id, unknown }
    }

    #[cfg(test)]
    pub(crate) fn of(tints: &[(u32, BiomeTint)]) -> BiomeColors {
        BiomeColors { by_id: tints.iter().copied().collect(), unknown: PLAINS }
    }

    /// Unknown ids (and chunks without biomes) tint like plains, with the files' default water.
    pub fn get(&self, id: u32) -> &BiomeTint {
        self.by_id.get(&id).unwrap_or(&self.unknown)
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

/// By biome name (namespace stripped); `default`'s water is for biomes not listed.
fn listed(root: &Path) -> FxHashMap<String, Listed> {
    let Ok(doc) = json::read(&root.join("biomes_client.json")) else { return FxHashMap::default() };
    let Some(biomes) = doc.get("biomes").and_then(Value::as_object) else { return FxHashMap::default() };
    let entry = |(name, b): (&String, &Value)| {
        let color = |key: &str| Some(rgb(u32::from_str_radix(b.get(key)?.as_str()?.strip_prefix('#')?, 16).ok()?));
        let listed = Listed {
            water: color("water_surface_color"),
            grass: color("grass_color"),
            grass_patch: color("grass_patch_color"),
            foliage: color("foliage_color"),
            dry_foliage: color("dry_foliage_color"),
        };
        (name.strip_prefix("minecraft:").unwrap_or(name).to_owned(), listed)
    };
    biomes.iter().map(entry).collect()
}
