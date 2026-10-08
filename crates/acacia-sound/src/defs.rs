//! Bedrock's sound tables: `sounds.json` (what a block, entity or world event plays) and
//! `sounds/sound_definitions.json` (which files a sound name picks from), plus each block's sound
//! type from `blocks.json`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// A sound name with its volume and pitch picked for one play.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub sound: String,
    pub volume: f32,
    pub pitch: f32,
}

/// One file a definition may play.
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    /// `sounds/dig/stone1`, without extension.
    pub path: String,
    pub volume: f32,
    pub pitch: f32,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Definition {
    pub files: Vec<File>,
    /// Blocks where it falls silent; `None` takes the default.
    pub max_distance: Option<f32>,
}

/// An event's sound with volume and pitch as ranges.
#[derive(Debug, Clone, PartialEq)]
struct Event {
    sound: String,
    volume: [f32; 2],
    pitch: [f32; 2],
}

type Events = HashMap<String, Event>;

#[derive(Debug, Default)]
pub struct Definitions {
    sounds: HashMap<String, Definition>,
    blocks: HashMap<String, Events>,
    entities: HashMap<String, Events>,
    entity_defaults: Events,
    individual: Events,
    /// Block name (no namespace) to its sound type.
    block_types: HashMap<String, String>,
}

impl Definitions {
    /// The tables under a resource pack `root`; missing files leave their part empty.
    pub fn load(root: &Path) -> Definitions {
        let read = |p: &str| std::fs::read_to_string(root.join(p)).ok().and_then(|t| serde_json::from_str::<Value>(&strip(&t)).ok()).unwrap_or(Value::Null);
        let (sounds, defs, blocks) = (read("sounds.json"), read("sounds/sound_definitions.json"), read("blocks.json"));
        let mut out = Definitions {
            sounds: defs["sound_definitions"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), definition(v))).collect(),
            entity_defaults: events(&sounds["entity_sounds"]["defaults"]),
            individual: events(&sounds["individual_event_sounds"]),
            block_types: blocks.as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), v["sound"].as_str()?.to_owned()))).collect(),
            ..Definitions::default()
        };
        // Interactive sounds (steps, falls) first, so the main table wins where both name an event.
        for table in [&sounds["interactive_sounds"]["block_sounds"], &sounds["block_sounds"]] {
            for (kind, group) in table.as_object().into_iter().flatten() {
                out.blocks.entry(kind.clone()).or_default().extend(events(group));
            }
        }
        for (kind, group) in sounds["entity_sounds"]["entities"].as_object().into_iter().flatten() {
            out.entities.insert(kind.clone(), events(group));
        }
        out
    }

    pub fn definition(&self, sound: &str) -> Option<&Definition> {
        self.sounds.get(sound)
    }

    /// What `event` (`place`, `break`, `hit`, `step`...) sounds like for the block `name`
    /// (`minecraft:stone`); blocks without a type sound like stone.
    pub fn block(&self, name: &str, event: &str, roll: impl FnMut() -> f32) -> Option<Cue> {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        let kind = self.block_types.get(name).map_or("stone", String::as_str);
        let events = self.blocks.get(kind).or_else(|| self.blocks.get("stone"))?;
        cue(events.get(event)?, roll)
    }

    /// An entity's `event` (`ambient`, `hurt`, `step`...), else the defaults'.
    pub fn entity(&self, kind: &str, event: &str, roll: impl FnMut() -> f32) -> Option<Cue> {
        let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);
        let event = self.entities.get(kind).and_then(|e| e.get(event)).or_else(|| self.entity_defaults.get(event))?;
        cue(event, roll)
    }

    /// A world event without a block or entity (`chest.open`, `explode`...).
    pub fn individual(&self, event: &str, roll: impl FnMut() -> f32) -> Option<Cue> {
        cue(self.individual.get(event)?, roll)
    }
}

fn cue(event: &Event, mut roll: impl FnMut() -> f32) -> Option<Cue> {
    let pick = |[lo, hi]: [f32; 2], r: f32| lo + (hi - lo) * r;
    (!event.sound.is_empty()).then(|| Cue { sound: event.sound.clone(), volume: pick(event.volume, roll()), pitch: pick(event.pitch, roll()) })
}

/// A group's events, each scaled by the group's own volume and pitch.
fn events(group: &Value) -> Events {
    let (volume, pitch) = (range(&group["volume"]), range(&group["pitch"]));
    let scale = |a: [f32; 2], b: [f32; 2]| [a[0] * b[0], a[1] * b[1]];
    group["events"].as_object().into_iter().flatten().filter_map(|(name, v)| {
        let event = match v {
            Value::String(s) => Event { sound: s.clone(), volume: [1.0; 2], pitch: [1.0; 2] },
            Value::Object(_) => Event { sound: v["sound"].as_str()?.to_owned(), volume: range(&v["volume"]), pitch: range(&v["pitch"]) },
            _ => return None,
        };
        Some((name.clone(), Event { volume: scale(event.volume, volume), pitch: scale(event.pitch, pitch), ..event }))
    }).collect()
}

/// A number or `[min, max]`; absent is 1.
fn range(v: &Value) -> [f32; 2] {
    match v {
        Value::Number(n) => [n.as_f64().unwrap_or(1.0) as f32; 2],
        Value::Array(a) => [0, 1].map(|i| a.get(i).or(a.first()).and_then(Value::as_f64).unwrap_or(1.0) as f32),
        _ => [1.0; 2],
    }
}

fn definition(v: &Value) -> Definition {
    let files = v["sounds"].as_array().into_iter().flatten().filter_map(|s| match s {
        Value::String(path) => Some(File { path: path.clone(), volume: 1.0, pitch: 1.0, weight: 1 }),
        Value::Object(_) => Some(File {
            path: s["name"].as_str()?.to_owned(),
            volume: s["volume"].as_f64().unwrap_or(1.0) as f32,
            pitch: s["pitch"].as_f64().unwrap_or(1.0) as f32,
            weight: s["weight"].as_u64().unwrap_or(1) as u32,
        }),
        _ => None,
    });
    Definition { files: files.collect(), max_distance: v["max_distance"].as_f64().map(|d| d as f32) }
}

/// Drops `//` comment lines, which the pack's files carry.
fn strip(text: &str) -> String {
    text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs assets/vanilla (tools/fetch-vanilla-pack.sh); skipped without it.
    fn pack() -> Option<Definitions> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
        dir.join("sounds.json").is_file().then(|| Definitions::load(&dir))
    }

    #[test]
    fn blocks_entities_and_events_resolve() {
        let Some(d) = pack() else { return };
        let mid = || 0.5;
        assert_eq!(d.block("minecraft:stone", "break", mid).map(|c| c.sound), Some("dig.stone".into()));
        assert_eq!(d.block("minecraft:oak_planks", "step", mid).map(|c| c.sound), Some("step.wood".into()));
        assert_eq!(d.entity("minecraft:cow", "hurt", mid).map(|c| c.sound), Some("mob.cow.hurt".into()));
        let click = d.definition("random.click").unwrap();
        assert_eq!(click.files[0].path, "sounds/random/click");
        assert!((click.files[0].volume - 0.2).abs() < 1e-6);
    }

    #[test]
    fn ranges_take_numbers_and_pairs() {
        assert_eq!(range(&serde_json::json!([0.8, 1.2])), [0.8, 1.2]);
        assert_eq!(range(&serde_json::json!(0.5)), [0.5, 0.5]);
        assert_eq!(range(&Value::Null), [1.0, 1.0]);
    }
}
