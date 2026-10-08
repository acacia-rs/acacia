//! FSB5 decoding of real Bedrock pack files (bedrock-samples v1.26.50.4), checked against
//! vgmstream-cli's WAV output for the same files.

use acacia_sound::fsb::{self, Pcm};
use std::path::Path;

fn fixture(name: &str) -> Pcm {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name);
    fsb::decode(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// FNV-1a 64 over the samples as little-endian bytes, as hashed from vgmstream's WAV data chunk.
fn fnv1a(samples: &[i16]) -> u64 {
    samples.iter().flat_map(|s| s.to_le_bytes()).fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

fn check(name: &str, rate: u32, channels: u16, len: usize, head: [i16; 8], hash: u64) {
    let pcm = fixture(name);
    assert_eq!((pcm.sample_rate, pcm.channels, pcm.samples.len()), (rate, channels, len), "{name}");
    assert_eq!(pcm.samples[..8], head, "{name}");
    assert_eq!(fnv1a(&pcm.samples), hash, "{name}");
}

#[test]
fn fadpcm_mono_matches_vgmstream() {
    check("wood_click.fsb", 37035, 1, 5888, [128, 372, 333, 414, 349, 277, 72, 5], 0x14ab_07f6_45a9_4844);
}

#[test]
fn fadpcm_stereo_matches_vgmstream() {
    check("click.fsb", 38320, 2, 21504, [0, 64, 64, -30, 34, -101, -3, -129], 0x421e_30c4_05e7_ba47);
}

#[test]
fn pcm16_matches_vgmstream() {
    check("turn3.fsb", 48000, 1, 2192, [0, 0, 0, 0, 0, 0, 1, 1], 0x2b1a_7429_249c_7ff6);
}

/// Decodes every `.fsb` under `$ACACIA_FSB_DIR` (e.g. bedrock-samples' resource_pack/sounds). If
/// `$ACACIA_FSB_REF` names a file of `<path relative to the dir> <fnv1a hex>` lines (from a reference
/// decoder's PCM), each listed file must match it too.
#[test]
fn every_file_in_acacia_fsb_dir() {
    let Some(dir) = std::env::var_os("ACACIA_FSB_DIR") else {
        eprintln!("ACACIA_FSB_DIR unset; skipping");
        return;
    };
    let dir = Path::new(&dir);
    let reference: std::collections::HashMap<String, u64> = std::env::var_os("ACACIA_FSB_REF")
        .map(|p| std::fs::read_to_string(p).unwrap())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(p, h)| (p.to_owned(), u64::from_str_radix(h.trim_start_matches("0x"), 16).unwrap()))
        .collect();
    let mut files = Vec::new();
    collect(dir, &mut files);
    assert!(!files.is_empty(), "no .fsb files under {dir:?}");
    let mut failures = Vec::new();
    let mut compared = 0;
    for path in &files {
        let rel = path.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
        match fsb::decode(&std::fs::read(path).unwrap()) {
            Ok(pcm) if reference.get(&rel).is_some_and(|&h| h != fnv1a(&pcm.samples)) => {
                failures.push(format!("{rel}: differs from reference"));
            }
            Ok(pcm) if plausible(&pcm) => compared += usize::from(reference.contains_key(&rel)),
            Ok(pcm) => failures.push(format!("{}: implausible {}Hz x{} len {}", path.display(), pcm.sample_rate, pcm.channels, pcm.samples.len())),
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    eprintln!("decoded {} files, {compared} matched the reference, {} failures", files.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn plausible(pcm: &Pcm) -> bool {
    let frames = pcm.samples.len() / usize::from(pcm.channels);
    // FMOD's "optimise sample rate" drops some pack sounds to ~3.7 kHz.
    (2000..=96000).contains(&pcm.sample_rate)
        && (1..=2).contains(&pcm.channels)
        && frames > 0
        && frames <= pcm.sample_rate as usize * 600
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "fsb") {
            out.push(path);
        }
    }
}
