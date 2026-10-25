//! Java's sound effects for a Java look: the `.ogg` files of the version's asset index under
//! `minecraft/sounds/`, without music (unless asked for), records and ambience (~60 MB), saved as
//! `sounds/<path>.ogg`.
//! Bedrock's sound definitions name the same paths for most sounds, so acacia-sound plays Java's
//! audio wherever a file of the path exists. Nothing here is redistributed.

use std::path::Path;

use serde_json::Value;

use crate::download::{Error, get, pinned};
use crate::jar;

pub(crate) const OBJECTS: &str = "https://resources.download.minecraft.net";
const PREFIX: &str = "minecraft/sounds/";
const LEFT_OUT: [&str; 3] = ["music/", "records/", "ambient/"];

/// Downloads what is missing into `out/sounds`; returns how many files the look has.
pub fn fetch(out: &Path) -> Result<usize, Error> {
    let package = jar::version_package()?;
    let index_url = package["assetIndex"]["url"].as_str().ok_or("version has no asset index")?;
    let index: Value = serde_json::from_slice(&get(index_url)?)?;
    let objects = index["objects"].as_object().ok_or("asset index has no objects")?;
    // The Bedrock pack ships no music either: `LOOKBAKE_MUSIC=1` fetches Java's (some 500 MB).
    let left_out = &LEFT_OUT[usize::from(std::env::var_os("LOOKBAKE_MUSIC").is_some())..];
    let mut count = 0;
    for (key, object) in objects {
        let Some(path) = key.strip_prefix(PREFIX).filter(|p| p.ends_with(".ogg") && !left_out.iter().any(|l| p.starts_with(l))) else { continue };
        let hash = object["hash"].as_str().ok_or_else(|| format!("{key} has no hash"))?;
        pinned(&out.join("sounds").join(path), hash, || get(&format!("{OBJECTS}/{}/{hash}", &hash[..2])))?;
        count += 1;
    }
    Ok(count)
}
