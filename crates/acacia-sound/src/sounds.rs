//! Sound events to voices: resolve the event to a definition, pick one of its files by weight,
//! decode it once (`.ogg` first, then the pack's `.fsb`, root by root) and play it at a place.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::defs::{Cue, Definitions, File};
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
        let Some((file, max_distance)) = self.pick(&cue.sound) else { return };
        let Some(pcm) = self.load(&file.path) else { return };
        tracing::debug!(sound = cue.sound, file = file.path, samples = pcm.samples.len(), "play");
        let volume = volume * cue.volume * file.volume;
        let range = max_distance.unwrap_or(DEFAULT_RANGE) * volume.max(1.0);
        self.output.play(Voice { pcm, volume, pitch: pitch * cue.pitch * file.pitch, position: at, range }, listener);
    }

    /// Plays one of a definition's long samples (music) everywhere: it is read and decoded on a
    /// thread of its own, so the caller does not wait for it, and is not kept afterwards.
    pub fn stream(&mut self, sound: &str, volume: f32) {
        let Some((file, _)) = self.pick(sound) else { return };
        let Some(mixer) = self.output.mixer() else { return };
        let roots = self.roots.clone();
        std::thread::spawn(move || {
            if let Some(pcm) = read(&roots, &file.path) {
                tracing::debug!(file = file.path, samples = pcm.samples.len(), "stream");
                crate::player::mix(&mixer, &pcm, (volume * file.volume).min(1.0), file.pitch, [1.0, 1.0]);
            }
        });
    }

    /// One of the files of the definition `sound`, by weight, and the definition's range.
    fn pick(&mut self, sound: &str) -> Option<(File, Option<f32>)> {
        let Some(def) = self.defs.definition(sound) else {
            tracing::debug!(sound, "no sound definition");
            return None;
        };
        let total: u32 = def.files.iter().map(|f| f.weight.max(1)).sum();
        let mut pick = (roll(&mut self.seed) * total as f32) as u32;
        let file = def.files.iter().find(|f| {
            let hit = pick < f.weight.max(1);
            pick = pick.saturating_sub(f.weight.max(1));
            hit
        })?;
        Some((file.clone(), def.max_distance))
    }

    fn load(&mut self, path: &str) -> Option<Arc<Pcm>> {
        if let Some(cached) = self.cache.get(path) {
            return cached.clone();
        }
        let pcm = read(&self.roots, path).map(Arc::new);
        self.cache.insert(path.to_owned(), pcm.clone());
        pcm
    }
}

/// The sample `path` from the first root that has it, `.ogg` before the pack's `.fsb`.
fn read(roots: &[PathBuf], path: &str) -> Option<Pcm> {
    let at = |root: &PathBuf, path: &String| {
        let ogg_file = root.join(format!("{path}.ogg"));
        if let Ok(bytes) = std::fs::read(&ogg_file) {
            tracing::debug!(file = %ogg_file.display(), "ogg");
            return ogg::decode(&bytes).inspect_err(|e| tracing::debug!(%e, file = %ogg_file.display(), "ogg")).ok();
        }
        let fsb_file = root.join(format!("{path}.fsb"));
        let bytes = std::fs::read(&fsb_file).ok()?;
        fsb::decode(&bytes).inspect_err(|e| tracing::debug!(%e, file = %fsb_file.display(), "fsb")).ok()
    };
    let names = crate::java_names::candidates(path);
    let pcm = roots.iter().find_map(|root| names.iter().find_map(|name| at(root, name)));
    if pcm.is_none() {
        tracing::debug!(path, "sound file missing or undecodable");
    }
    pcm
}

/// xorshift64: 0 to 1.
fn roll(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs assets/vanilla (tools/fetch-vanilla-pack.sh) and a Java look baked with
    /// `LOOKBAKE_MUSIC=1` (the Bedrock pack has no music files); skipped without them.
    #[test]
    fn a_music_track_decodes_to_minutes_of_sound() {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let roots = [assets.join("vanilla"), assets.join("looks/java")];
        let defs = Definitions::load(&roots[0]);
        let Some(def) = defs.definition("music.game") else { return };
        let names = crate::java_names::candidates(&def.files[0].path);
        if !roots.iter().any(|root| names.iter().any(|name| root.join(format!("{name}.ogg")).is_file())) {
            return println!("no music files");
        }
        let pcm = read(&roots, &def.files[0].path).expect("the first track decodes");
        let seconds = pcm.samples.len() as f32 / (pcm.sample_rate as f32 * f32::from(pcm.channels));
        println!("music track {}: {seconds:.0} s, {} Hz, {} channels", def.files[0].path, pcm.sample_rate, pcm.channels);
        assert!(seconds > 30.0 && pcm.samples.iter().any(|&s| s != 0), "{seconds} s");
    }
}
