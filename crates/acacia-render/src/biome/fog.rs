//! Fog with the camera in a fluid, from a look's files. Bedrock's pack names a fog per biome in
//! `biomes_client.json` (`fog_identifier`) and defines it in `fogs/*.json`; a Java look lists
//! `water_fog_color` and `water_fog_distance` per biome there instead.

use std::path::Path;

use rustc_hash::FxHashMap;
use serde_json::Value;

use crate::assets::json;

/// Colour and distances of one fog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// sRGB.
    pub color: [u8; 3],
    pub start: f32,
    pub end: f32,
    /// `start` and `end` are shares of the render distance, not blocks (`render_distance_type`).
    pub relative: bool,
}

/// Bedrock's `transition_fog`: the fog closes in from `init` over the first seconds in water.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub init: Fog,
    pub min_percent: f32,
    pub mid_seconds: f32,
    pub mid_percent: f32,
    pub max_seconds: f32,
}

impl Transition {
    /// How far from `init` to the fog after `seconds`: `min_percent` at once, `mid_percent` at
    /// `mid_seconds`, all of it at `max_seconds`, linear in between.
    pub fn progress(&self, seconds: f32) -> f32 {
        if seconds < self.mid_seconds {
            lerp(self.min_percent, self.mid_percent, seconds / self.mid_seconds)
        } else {
            lerp(self.mid_percent, 1.0, ((seconds - self.mid_seconds) / (self.max_seconds - self.mid_seconds)).min(1.0))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterFog {
    pub fog: Fog,
    pub transition: Option<Transition>,
}

/// Fog in water per biome id, and in lava and powder snow.
#[derive(Debug, Clone)]
pub struct FluidFogs {
    water: FxHashMap<u32, WaterFog>,
    default_water: WaterFog,
    /// The air's fog colour for biomes that set one (sRGB): what tells the Nether's biomes apart.
    air: FxHashMap<u32, [u8; 3]>,
    pub lava: Fog,
    pub powder_snow: Fog,
}

const fn fixed(color: [u8; 3], start: f32, end: f32) -> Fog {
    Fog { color, start, end, relative: false }
}

/// `fog_default` and `fog_powder_snow` of the 1.26.50 pack, for a look without `fogs/`.
const BEDROCK_WATER: WaterFog = WaterFog {
    fog: fixed([0x44, 0xAF, 0xF5], 0.0, 60.0),
    transition: Some(Transition { init: fixed([0x44, 0xAF, 0xF5], 0.0, 0.01), min_percent: 0.25, mid_seconds: 5.0, mid_percent: 0.6, max_seconds: 30.0 }),
};
const BEDROCK_LAVA: Fog = fixed([0x99, 0x1A, 0x00], 0.0, 0.64);
const BEDROCK_POWDER_SNOW: Fog = fixed([0x9F, 0xBB, 0xC8], 0.0, 2.0);

impl Default for FluidFogs {
    fn default() -> Self {
        FluidFogs { water: FxHashMap::default(), default_water: BEDROCK_WATER, air: FxHashMap::default(), lava: BEDROCK_LAVA, powder_snow: BEDROCK_POWDER_SNOW }
    }
}

impl FluidFogs {
    /// `biomes` are the world's ids and names (namespace stripped).
    pub fn load<'a>(root: &Path, biomes: impl Iterator<Item = (u32, &'a str)>) -> FluidFogs {
        let defined = definitions(root);
        let listed = json::read(&root.join("biomes_client.json")).ok();
        let listed = listed.as_ref().and_then(|d| d.get("biomes")).and_then(Value::as_object);
        let entry = |name: &str| listed.and_then(|l| l.get(name).or_else(|| l.get(&format!("minecraft:{name}"))));
        let fog_of = |entry: Option<&Value>| entry.and_then(|e| e.get("fog_identifier")?.as_str()).and_then(|id| defined.get(id));
        let default_fog = fog_of(entry("default"));
        let distance = |key: &str| default_fog.and_then(|f| fog(f.get(key)?));
        let water_of = |entry: Option<&Value>| java_water(entry?).or_else(|| water(fog_of(entry)?.get("water")?));
        let default_water = water_of(entry("default")).unwrap_or(BEDROCK_WATER);
        // A Java look lists the colour itself; Bedrock's is its fog definition's `air`.
        let air_of = |entry: Option<&Value>| entry.and_then(|e| color(e.get("fog_color")?)).or_else(|| color(fog_of(entry)?.get("air")?.get("fog_color")?));
        let (mut water, mut air) = (FxHashMap::default(), FxHashMap::default());
        for (id, name) in biomes {
            water.extend(water_of(entry(name)).map(|fog| (id, fog)));
            air.extend(air_of(entry(name)).map(|color| (id, color)));
        }
        let powder_snow = defined.get("minecraft:fog_powder_snow").and_then(|f| fog(f.get("powder_snow")?));
        FluidFogs { water, default_water, air, lava: distance("lava").unwrap_or(BEDROCK_LAVA), powder_snow: powder_snow.unwrap_or(BEDROCK_POWDER_SNOW) }
    }

    /// The air's fog colour in `biome`, where it has its own.
    pub fn air(&self, biome: Option<u32>) -> Option<[u8; 3]> {
        self.air.get(&biome?).copied()
    }

    pub fn water(&self, biome: Option<u32>) -> &WaterFog {
        biome.and_then(|b| self.water.get(&b)).unwrap_or(&self.default_water)
    }
}

/// Each `fogs/*.json` definition's `distance` block by identifier.
fn definitions(root: &Path) -> FxHashMap<String, Value> {
    let Ok(dir) = std::fs::read_dir(root.join("fogs")) else { return FxHashMap::default() };
    let settings = |path: &Path| {
        let mut doc = json::read(path).ok()?;
        let settings = doc.get_mut("minecraft:fog_settings")?;
        let id = settings.get("description")?.get("identifier")?.as_str()?.to_owned();
        Some((id, settings.get_mut("distance")?.take()))
    };
    dir.filter_map(|e| settings(&e.ok()?.path())).collect()
}

fn fog(v: &Value) -> Option<Fog> {
    let number = |key: &str| Some(v.get(key)?.as_f64()? as f32);
    let relative = v.get("render_distance_type").and_then(Value::as_str) == Some("render");
    Some(Fog { color: color(v.get("fog_color")?)?, start: number("fog_start")?, end: number("fog_end")?, relative })
}

fn water(v: &Value) -> Option<WaterFog> {
    let transition = v.get("transition_fog").and_then(|t| {
        let number = |key: &str| Some(t.get(key)?.as_f64()? as f32);
        Some(Transition {
            init: fog(t.get("init_fog")?)?,
            min_percent: number("min_percent")?,
            mid_seconds: number("mid_seconds")?,
            mid_percent: number("mid_percent")?,
            max_seconds: number("max_seconds")?,
        })
    });
    Some(WaterFog { fog: fog(v)?, transition })
}

/// A Java look's entry: colour and distances, no transition.
fn java_water(entry: &Value) -> Option<WaterFog> {
    let distance = entry.get("water_fog_distance")?.as_array()?;
    let at = |i: usize| Some(distance.get(i)?.as_f64()? as f32);
    Some(WaterFog { fog: fixed(color(entry.get("water_fog_color")?)?, at(0)?, at(1)?), transition: None })
}

fn color(v: &Value) -> Option<[u8; 3]> {
    let v = u32::from_str_radix(v.as_str()?.strip_prefix('#')?, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("acacia-fog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            std::fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
            std::fs::write(dir.join(path), text).unwrap();
        }
        dir
    }

    #[test]
    fn bedrock_biomes_take_their_fog_and_the_default_fills_in() {
        let fog = |id: &str, distance: &str| format!(r#"{{"minecraft:fog_settings": {{"description": {{"identifier": "{id}"}}, "distance": {{{distance}}}}}}}"#);
        let default = fog("minecraft:fog_default", r##""water": {"fog_start": 0, "fog_end": 60, "fog_color": "#44AFF5", "render_distance_type": "fixed"}, "lava": {"fog_start": 0, "fog_end": 0.64, "fog_color": "#991A00", "render_distance_type": "fixed"}"##);
        let swamp = fog("minecraft:fog_swampland", r##""water": {"fog_start": 0, "fog_end": 30, "fog_color": "#232317", "render_distance_type": "fixed"}"##);
        let client = r#"{"biomes": {"default": {"fog_identifier": "minecraft:fog_default"}, "swampland": {"fog_identifier": "minecraft:fog_swampland"}, "plains": {"fog_identifier": "minecraft:fog_plains"}}}"#;
        let root = dir("bedrock", &[("fogs/default.json", &default), ("fogs/swamp.json", &swamp), ("biomes_client.json", client)]);
        let fogs = FluidFogs::load(&root, [(6, "swampland"), (1, "plains")].into_iter());
        assert_eq!(fogs.water(Some(6)).fog, fixed([0x23, 0x23, 0x17], 0.0, 30.0));
        assert_eq!(fogs.water(Some(1)).fog, fixed([0x44, 0xAF, 0xF5], 0.0, 60.0));
        assert_eq!((fogs.lava, fogs.powder_snow), (BEDROCK_LAVA, BEDROCK_POWDER_SNOW));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn java_entries_carry_their_own_water_fog() {
        let client = r##"{"biomes": {"default": {"water_fog_color": "#050533", "water_fog_distance": [-8, 96]}, "minecraft:swampland": {"water_fog_color": "#232317", "water_fog_distance": [-8, 81.6]}}}"##;
        let root = dir("java", &[("biomes_client.json", client)]);
        let fogs = FluidFogs::load(&root, [(6, "swampland")].into_iter());
        assert_eq!(fogs.water(Some(6)).fog, fixed([0x23, 0x23, 0x17], -8.0, 81.6));
        assert_eq!(*fogs.water(None), WaterFog { fog: fixed([5, 5, 0x33], -8.0, 96.0), transition: None });
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn nether_biomes_have_their_own_air_fog_in_both_packs() {
        let crimson = r##"{"minecraft:fog_settings": {"description": {"identifier": "minecraft:fog_crimson_forest"}, "distance": {"air": {"fog_start": 10, "fog_end": 96, "fog_color": "#330303", "render_distance_type": "fixed"}}}}"##;
        let bedrock = r#"{"biomes": {"crimson_forest": {"fog_identifier": "minecraft:fog_crimson_forest"}, "plains": {}}}"#;
        let root = dir("air-bedrock", &[("fogs/crimson.json", crimson), ("biomes_client.json", bedrock)]);
        let fogs = FluidFogs::load(&root, [(179, "crimson_forest"), (1, "plains")].into_iter());
        assert_eq!((fogs.air(Some(179)), fogs.air(Some(1)), fogs.air(None)), (Some([0x33, 3, 3]), None, None));
        std::fs::remove_dir_all(&root).unwrap();
        let java = r##"{"biomes": {"minecraft:warped_forest": {"fog_color": "#1a051a"}}}"##;
        let root = dir("air-java", &[("biomes_client.json", java)]);
        assert_eq!(FluidFogs::load(&root, [(180, "warped_forest")].into_iter()).air(Some(180)), Some([0x1A, 5, 0x1A]));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_transition_reaches_mid_then_full() {
        let t = BEDROCK_WATER.transition.unwrap();
        assert_eq!((t.progress(0.0), t.progress(5.0), t.progress(30.0), t.progress(99.0)), (0.25, 0.6, 1.0, 1.0));
        assert!((t.progress(17.5) - 0.8).abs() < 1e-6);
    }
}
