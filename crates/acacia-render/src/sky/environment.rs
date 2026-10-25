//! `EnvironmentAttributeSystem` from a Java look's `environment.json` (written by `lookbake java`
//! from the jar's dimension types, timelines and biomes): the dimension's values, the biomes'
//! blended over the camera ([`super::blend`]), the dimension's timelines, then the weather.

use std::path::Path;

use rustc_hash::FxHashMap;
use serde_json::Value;

use super::attributes::{Attr, Attributes};
use super::timeline::{Modifier, Timeline, Value4, parse_value};
use crate::assets::json;

/// What a dimension draws as its sky (`DimensionType.Skybox`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skybox {
    None,
    Overworld,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DimensionEnv {
    values: Vec<(Attr, Value4)>,
    timelines: Vec<Timeline>,
    /// Sky light and no ceiling: rain and thunder change the attributes.
    pub weather: bool,
    pub skybox: Skybox,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Environment {
    dimensions: FxHashMap<String, DimensionEnv>,
    /// Which of `layers` a biome (Bedrock's name, namespace stripped) takes.
    biomes: FxHashMap<String, usize>,
    /// One per Java biome with an `attributes` map: the interpolator weighs each map once.
    layers: Vec<Vec<(Attr, Modifier, Value4)>>,
}

/// Where biomes without an attributes map land (`EnvironmentAttributeMap.EMPTY`, shared).
const EMPTY: usize = usize::MAX;

impl Environment {
    /// `None` without the file: the look predates it, or it is Bedrock's.
    pub fn load(files: &Path) -> Option<Environment> {
        Some(Environment::parse(&json::read(&files.join("environment.json")).ok()?))
    }

    pub fn parse(doc: &Value) -> Environment {
        let timelines: FxHashMap<&str, Timeline> =
            doc["timelines"].as_object().into_iter().flatten().map(|(name, t)| (strip(name), Timeline::parse(t))).collect();
        let dimension = |d: &Value| DimensionEnv {
            values: d["attributes"].as_object().into_iter().flatten().filter_map(|(k, v)| Some((Attr::by_name(k)?, parse_value(v)?))).collect(),
            timelines: d["timelines"].as_array().into_iter().flatten().filter_map(|t| timelines.get(strip(t.as_str()?)).cloned()).collect(),
            weather: d["has_skylight"].as_bool().unwrap_or(false) && !d["has_ceiling"].as_bool().unwrap_or(false),
            skybox: match d["skybox"].as_str().unwrap_or("overworld") {
                "none" => Skybox::None,
                "end" => Skybox::End,
                _ => Skybox::Overworld,
            },
        };
        let dimensions = doc["dimensions"].as_object().into_iter().flatten().map(|(name, d)| (strip(name).to_owned(), dimension(d))).collect();
        let layer = |(k, v): (&String, &Value)| -> Option<(Attr, Modifier, Value4)> {
            let attr = Attr::by_name(k)?;
            match parse_value(v) {
                Some(value) => Some((attr, Modifier::Override, value)),
                None => Some((attr, Modifier::parse(v["modifier"].as_str())?, parse_value(&v["argument"])?)),
            }
        };
        let (mut biomes, mut layers) = (FxHashMap::default(), Vec::new());
        for (index, b) in doc["biomes"].as_object().into_iter().flatten().map(|(_, b)| b).enumerate() {
            layers.push(b["attributes"].as_object().into_iter().flatten().filter_map(layer).collect());
            biomes.extend(b["bedrock"].as_array().into_iter().flatten().filter_map(|n| Some((strip(n.as_str()?).to_owned(), index))));
        }
        Environment { dimensions, biomes, layers }
    }

    /// By Java's name: `overworld`, `the_nether`, `the_end`.
    pub fn dimension(&self, name: &str) -> Option<&DimensionEnv> {
        self.dimensions.get(name)
    }

    /// The attributes at `ticks` of the dimension's clock, `weights` the camera's biomes (by
    /// Bedrock name) and their weights ([`super::blend::gaussian_weights`]), `rain` and
    /// `thunder` 0..1, `flash` while a lightning flash lasts.
    pub fn sample(&self, dimension: &DimensionEnv, ticks: i64, weights: &[(&str, f64)], rain: f32, thunder: f32, flash: bool) -> Attributes {
        let mut sources: Vec<(usize, f64)> = Vec::new();
        for &(biome, w) in weights {
            let key = self.biomes.get(biome).copied().unwrap_or(EMPTY);
            match sources.iter_mut().find(|(k, _)| *k == key) {
                Some((_, total)) => *total += w,
                None => sources.push((key, w)),
            }
        }
        let mut out = Attributes::default();
        for &attr in Attr::ALL {
            let mut v = dimension.values.iter().find_map(|(a, v)| (*a == attr).then_some(*v)).unwrap_or(attr.default());
            if !sources.is_empty() && self.layers.iter().any(|l| l.iter().any(|(a, ..)| *a == attr)) {
                let base = v;
                v = super::blend::blend(sources.iter().map(|&(key, w)| (self.biome_value(key, attr, base), w)));
            }
            for timeline in &dimension.timelines {
                if let Some(track) = timeline.track(attr.name()) {
                    v = track.modifier.apply(v, track.sample(ticks, timeline.period));
                }
            }
            out.set(attr, v);
        }
        if dimension.weather {
            out.apply_weather(rain, thunder);
        }
        if flash {
            out.apply_flash();
        }
        out.sanitize();
        out
    }

    fn biome_value(&self, key: usize, attr: Attr, base: Value4) -> Value4 {
        let layer = self.layers.get(key).and_then(|l| l.iter().find(|(a, ..)| *a == attr));
        layer.map_or(base, |(_, modifier, argument)| modifier.apply(base, *argument))
    }
}

fn strip(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_doc() -> Value {
        json!({
            "timelines": {"minecraft:day": {"period_ticks": 24000, "tracks": {
                "minecraft:visual/sky_color": {"keyframes": [{"ticks": 133, "value": "#ffffff"}, {"ticks": 11867, "value": "#ffffff"}, {"ticks": 13670, "value": "#000000"}, {"ticks": 22330, "value": "#000000"}], "modifier": "multiply"},
                "minecraft:gameplay/sky_light_level": {"keyframes": [{"ticks": 133, "value": 1.0}, {"ticks": 11867, "value": 1.0}, {"ticks": 13670, "value": 0.26666668}, {"ticks": 22330, "value": 0.26666668}], "modifier": "multiply"}
            }}},
            "dimensions": {
                "minecraft:overworld": {"attributes": {"minecraft:visual/sky_color": "#78a7ff", "minecraft:visual/fog_color": "#c0d8ff"}, "timelines": ["minecraft:day"], "has_skylight": true, "has_ceiling": false},
                "minecraft:the_nether": {"attributes": {"minecraft:visual/fog_end_distance": 96.0}, "timelines": [], "has_skylight": false, "has_ceiling": true, "skybox": "none"}
            },
            "biomes": {
                "minecraft:swamp": {"bedrock": ["swampland", "swampland_mutated"], "attributes": {"minecraft:visual/water_fog_end_distance": {"argument": 0.85, "modifier": "multiply"}, "minecraft:visual/sky_color": "#000000"}}
            }
        })
    }

    #[test]
    fn the_day_darkens_the_dimensions_sky() {
        let env = Environment::parse(&sample_doc());
        let overworld = env.dimension("overworld").unwrap();
        let noon = env.sample(overworld, 6000, &[], 0.0, 0.0, false);
        assert_eq!(noon.rgb(Attr::SkyColor), [0x78 as f32 / 255.0, 0xA7 as f32 / 255.0, 1.0]);
        let midnight = env.sample(overworld, 18000 + 24000 * 5, &[], 0.0, 0.0, false);
        assert_eq!(midnight.rgb(Attr::SkyColor), [0.0; 3]);
        assert!((midnight.darken() - 11.0).abs() < 1e-4);
        let nether = env.dimension("the_nether").unwrap();
        assert_eq!((nether.skybox, nether.weather), (Skybox::None, false));
        assert_eq!(env.sample(nether, 0, &[], 1.0, 1.0, false).number(Attr::FogEnd), 96.0);
    }

    #[test]
    fn biomes_blend_by_weight_over_the_dimension() {
        let env = Environment::parse(&sample_doc());
        let overworld = env.dimension("overworld").unwrap();
        let half = env.sample(overworld, 6000, &[("plains", 1.0), ("swampland", 1.0)], 0.0, 0.0, false);
        assert!((half.rgb(Attr::SkyColor)[2] - 0.5).abs() < 1e-6);
        assert!((half.number(Attr::WaterFogEnd) - 96.0 * (1.0 + 0.85) / 2.0).abs() < 1e-4);
        // Two Bedrock names of one Java biome weigh as one source: a third of plains is left.
        let third = env.sample(overworld, 6000, &[("plains", 1.0), ("swampland", 1.0), ("swampland_mutated", 1.0)], 0.0, 0.0, false);
        assert!((third.rgb(Attr::SkyColor)[2] - 1.0 / 3.0).abs() < 1e-6);
        let flash = env.sample(overworld, 18000, &[], 0.0, 0.0, true);
        assert_eq!((flash.rgb(Attr::SkyColor), flash.number(Attr::SkyLightFactor)), ([0.8 * 0.22, 0.8 * 0.22, 0.22], 1.0));
    }
}
