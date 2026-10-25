//! Biome tint colours. Grass and foliage come from the vanilla colormaps indexed by the server's
//! temperature and downfall (`BiomeDefinitionList`), with vanilla's hard-coded exceptions; water
//! comes from `biomes_client.json`.

pub mod fog;
pub mod ids;
pub mod noise;

use std::path::Path;

use rustc_hash::FxHashMap;
use serde_json::Value;

use crate::assets::json;
use crate::weather::climate::Climate;

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
    climate: Option<Climate>,
}

pub(crate) const PLAINS_CLIMATE: Climate = Climate { temperature: 0.8, frozen: false, has_precipitation: true };

pub struct BiomeColors {
    by_id: FxHashMap<u32, BiomeTint>,
    unknown: BiomeTint,
    climate: FxHashMap<u32, Climate>,
    fluid_fogs: fog::FluidFogs,
    names: FxHashMap<u32, String>,
    /// A Java look's environment attributes ([`crate::sky::Environment::load`]).
    environment: Option<std::sync::Arc<crate::sky::Environment>>,
}

impl Default for BiomeColors {
    /// Plains everywhere, until the server's definitions arrive.
    fn default() -> Self {
        BiomeColors {
            by_id: FxHashMap::default(),
            unknown: PLAINS,
            climate: FxHashMap::default(),
            fluid_fogs: Default::default(),
            names: FxHashMap::default(),
            environment: None,
        }
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
                let id = id_of(d)?;
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
        // The look's Java climate where it lists one; the server's has no `has_precipitation`, so
        // no downfall stands for it (deserts, savannas, badlands).
        let server = |d: &BiomeDef| Climate { temperature: d.temperature, frozen: false, has_precipitation: d.downfall > 0.0 };
        let climate = defs.iter().filter_map(|d| Some((u32::from(id_of(d)?), listed.get(d.name.as_str()).and_then(|l| l.climate).unwrap_or_else(|| server(d))))).collect();
        let fluid_fogs = fog::FluidFogs::load(root, defs.iter().filter_map(|d| Some((u32::from(id_of(d)?), d.name.as_str()))));
        let names = defs.iter().filter_map(|d| Some((u32::from(id_of(d)?), d.name.clone()))).collect();
        let environment = crate::sky::Environment::load(root).map(std::sync::Arc::new);
        BiomeColors { by_id, unknown, climate, fluid_fogs, names, environment }
    }

    #[cfg(test)]
    pub(crate) fn of(tints: &[(u32, BiomeTint)]) -> BiomeColors {
        BiomeColors { by_id: tints.iter().copied().collect(), ..Default::default() }
    }

    pub fn fluid_fogs(&self) -> &fog::FluidFogs {
        &self.fluid_fogs
    }

    pub fn environment(&self) -> Option<&crate::sky::Environment> {
        self.environment.as_deref()
    }

    /// The server's name for biome `id`, without namespace.
    pub fn name(&self, id: u32) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    /// The climate of `id`; unknown ids get plains'.
    pub fn climate(&self, id: u32) -> Climate {
        self.climate.get(&id).copied().unwrap_or(PLAINS_CLIMATE)
    }

    /// Unknown ids (and chunks without biomes) tint like plains, with the files' default water.
    pub fn get(&self, id: u32) -> &BiomeTint {
        self.by_id.get(&id).unwrap_or(&self.unknown)
    }
}

/// A definition's numeric id: its own, or vanilla's for its name.
fn id_of(d: &BiomeDef) -> Option<u16> {
    if d.id == ids::UNSET { ids::vanilla(&d.name) } else { Some(d.id) }
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
            climate: b.get("temperature").and_then(Value::as_f64).map(|t| Climate {
                temperature: t as f32,
                frozen: b.get("temperature_modifier").and_then(Value::as_str) == Some("frozen"),
                has_precipitation: b.get("has_precipitation").and_then(Value::as_bool).unwrap_or(true),
            }),
        };
        (name.strip_prefix("minecraft:").unwrap_or(name).to_owned(), listed)
    };
    biomes.iter().map(entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_looks_java_climate_wins_over_the_servers() {
        let def = |id, name: &str, temperature, downfall| BiomeDef { id, name: name.to_owned(), temperature, downfall };
        let dir = std::env::temp_dir().join(format!("acacia-climate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let listed = r#"{"biomes": {"minecraft:frozen_ocean": {"temperature": 0.0, "temperature_modifier": "frozen", "has_precipitation": true}, "minecraft:savanna": {"temperature": 2.0, "has_precipitation": false}}}"#;
        std::fs::write(dir.join("biomes_client.json"), listed).unwrap();
        let biomes = BiomeColors::build(&[def(1, "plains", 0.8, 0.4), def(2, "desert", 2.0, 0.0), def(10, "frozen_ocean", 0.5, 0.5), def(35, "savanna", 1.2, 0.4)], &dir);
        let climate = |id| biomes.climate(id);
        assert_eq!((climate(1).has_precipitation, climate(2).has_precipitation, climate(35).has_precipitation), (true, false, false));
        assert_eq!(climate(10), Climate { temperature: 0.0, frozen: true, has_precipitation: true });
        assert_eq!(climate(99), PLAINS_CLIMATE);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
