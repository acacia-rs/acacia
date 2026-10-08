//! Sound events to voices: resolve the event to a definition, pick one of its files by weight,
//! decode it once (`.ogg` first, then the pack's `.fsb`, root by root) and play it at a place.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::defs::{Cue, Definitions};
use crate::fsb::{self, Pcm};
use crate::ogg;
use crate::player::{Listener, Output, Voice};

/// Range of a sound whose definition gives none (Java's 16 blocks).
const DEFAULT_RANGE: f32 = 16.0;

pub struct Sounds {
    defs: Definitions,
    /// Where sample files are looked for, in order; each holds `sounds/...`.
    roots: Vec<PathBuf>,
    cache: HashMap<String, Option<Arc<Pcm>>>,
    output: Output,
    seed: u64,
}

/// What to play.
pub enum Event<'a> {
    /// A sound definition by name (`random.click`), as `PlaySound` names it.
    Named(&'a str),
    /// A block's event (`place`, `break`, `hit`, `step`), by block name.
    Block { block: &'a str, event: &'a str },
    Entity { kind: &'a str, event: &'a str },
    /// A world event (`chest.open`).
    World(&'a str),
}

impl Sounds {
    /// `defs_root` holds the Bedrock tables; `roots` the sample files, preferred first.
    pub fn new(defs_root: &std::path::Path, roots: Vec<PathBuf>) -> Sounds {
        Sounds { defs: Definitions::load(defs_root), roots, cache: HashMap::new(), output: Output::open(), seed: 0x9E37_79B9_7F4A_7C15 }
    }

    /// Plays `event` at `at` (`None`: not placed), its volume and pitch scaled.
    pub fn play(&mut self, event: Event, at: Option<[f64; 3]>, volume: f32, pitch: f32, listener: &Listener) {
        let mut seed = self.seed;
        let mut next = || roll(&mut seed);
        let cue = match event {
            Event::Named(name) => Some(Cue { sound: name.to_owned(), volume: 1.0, pitch: 1.0 }),
            Event::Block { block, event } => self.defs.block(block, event, &mut next),
            Event::Entity { kind, event } => self.defs.entity(kind, event, &mut next),
            Event::World(event) => self.defs.individual(event, &mut next),
        };
        self.seed = seed;
        let Some(cue) = cue else { return };
        let Some(def) = self.defs.definition(&cue.sound).cloned() else {
            tracing::debug!(sound = cue.sound, "no sound definition");
            return;
        };
        let total: u32 = def.files.iter().map(|f| f.weight.max(1)).sum();
        let mut pick = (roll(&mut self.seed) * total as f32) as u32;
        let Some(file) = def.files.iter().find(|f| {
            let hit = pick < f.weight.max(1);
            pick = pick.saturating_sub(f.weight.max(1));
            hit
        }) else {
            return;
        };
        let Some(pcm) = self.load(&file.path) else { return };
        tracing::debug!(sound = cue.sound, file = file.path, samples = pcm.samples.len(), "play");
        let volume = volume * cue.volume * file.volume;
        let range = def.max_distance.unwrap_or(DEFAULT_RANGE) * volume.max(1.0);
        self.output.play(Voice { pcm, volume, pitch: pitch * cue.pitch * file.pitch, position: at, range }, listener);
    }

    fn load(&mut self, path: &str) -> Option<Arc<Pcm>> {
        if let Some(cached) = self.cache.get(path) {
            return cached.clone();
        }
        let pcm = self.roots.iter().find_map(|root| {
            let ogg_file = root.join(format!("{path}.ogg"));
            if let Ok(bytes) = std::fs::read(&ogg_file) {
                return ogg::decode(&bytes).inspect_err(|e| tracing::debug!(%e, file = %ogg_file.display(), "ogg")).ok();
            }
            let fsb_file = root.join(format!("{path}.fsb"));
            let bytes = std::fs::read(&fsb_file).ok()?;
            fsb::decode(&bytes).inspect_err(|e| tracing::debug!(%e, file = %fsb_file.display(), "fsb")).ok()
        });
        if pcm.is_none() {
            tracing::debug!(path, "sound file missing or undecodable");
        }
        let pcm = pcm.map(Arc::new);
        self.cache.insert(path.to_owned(), pcm.clone());
        pcm
    }
}

/// xorshift64: 0 to 1.
fn roll(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 40) as f32 / (1u64 << 24) as f32
}
