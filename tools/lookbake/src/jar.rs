//! The Java Edition client jar: downloaded from Mojang's version manifest and unpacked to the
//! assets a Java look is baked from. Nothing here is redistributed; it runs on the user's machine.

use std::fs::File;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::download::{Error, get, pinned};

/// The Java release of the Bedrock drop the block palette is from (26.50, "Wilderness Bound").
pub const VERSION: &str = "26.3";
/// `downloads.client.sha1` of that release.
const CLIENT_SHA1: &str = "e877b6a07acd633fb3bb475002175cec036e7b87";
const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const ASSETS: &str = "assets/minecraft/";
const DATA: &str = "data/minecraft/";
const BIOMES: &str = "data/minecraft/worldgen/biome/";
const BLOCK_TAGS: &str = "data/minecraft/tags/block/";
/// What is unpacked: assets, biomes, block tags, and what the environment attributes are read from
/// ([`crate::environment::write`]), and armour trims' materials and patterns (`entities::bake`).
const PARTS: [&str; 8] = [
    ASSETS,
    BIOMES,
    BLOCK_TAGS,
    "data/minecraft/timeline/",
    "data/minecraft/tags/timeline/",
    "data/minecraft/dimension_type/",
    "data/minecraft/trim_material/",
    "data/minecraft/trim_pattern/",
];

pub struct Jar {
    /// `assets/minecraft`.
    pub assets: PathBuf,
    /// `data/minecraft`: the biomes, timelines and dimension types.
    pub data: PathBuf,
    /// `data/minecraft/worldgen/biome`.
    pub biomes: PathBuf,
    /// `data/minecraft/tags/block`.
    pub block_tags: PathBuf,
}

/// `$ACACIA_JAVA_ASSETS`, else `assets/java` under the working directory.
pub fn default_dir() -> PathBuf {
    std::env::var_os("ACACIA_JAVA_ASSETS").map_or_else(|| PathBuf::from("assets/java"), PathBuf::from)
}

/// Downloads the pinned client jar into `dir` unless it is there already, and unpacks its assets,
/// biomes and block tags to `dir/<version>`.
pub fn fetch(dir: &Path) -> Result<Jar, Error> {
    let jar = dir.join(format!("client-{VERSION}.jar"));
    let out = dir.join(VERSION);
    // An unpack from before a part was taken lacks its directory.
    if !out.join("VERSION").is_file() || PARTS.iter().any(|part| !out.join(part).is_dir()) {
        pinned(&jar, CLIENT_SHA1, download_client)?;
        let files = unpack(&jar, &out)?;
        std::fs::write(out.join("VERSION"), VERSION)?;
        println!("{}: {files} files", out.display());
    }
    Ok(Jar { assets: out.join(ASSETS), data: out.join(DATA), biomes: out.join(BIOMES), block_tags: out.join(BLOCK_TAGS) })
}

/// The pinned version's package JSON (downloads, asset index).
pub fn version_package() -> Result<Value, Error> {
    let manifest: Value = serde_json::from_slice(&get(MANIFEST)?)?;
    let versions = manifest["versions"].as_array().ok_or("version manifest has no versions")?;
    let version = versions.iter().find(|v| v["id"] == VERSION).ok_or_else(|| format!("no Java {VERSION} in the version manifest"))?;
    Ok(serde_json::from_slice(&get(version["url"].as_str().ok_or("version has no url")?)?)?)
}

fn download_client() -> Result<Vec<u8>, Error> {
    let package = version_package()?;
    let url = package["downloads"]["client"]["url"].as_str().ok_or("client download has no url")?;
    println!("downloading {url}");
    get(url)
}

/// Unpacks the jar's [`PARTS`] under `out`; returns the file count.
fn unpack(jar: &Path, out: &Path) -> Result<usize, Error> {
    let mut archive = zip::ZipArchive::new(File::open(jar)?)?;
    let mut files = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        // `enclosed_name` drops entries that would land outside `out`.
        let Some(path) = entry.enclosed_name().filter(|p| entry.is_file() && PARTS.iter().any(|part| p.starts_with(part))) else { continue };
        let to = out.join(path);
        std::fs::create_dir_all(to.parent().expect("an assets path has a parent"))?;
        std::io::copy(&mut entry, &mut File::create(to)?)?;
        files += 1;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn only_assets_biomes_block_tags_and_environment_data_are_unpacked() {
        let dir = std::env::temp_dir().join(format!("lookbake-jar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("client.jar");
        let mut zip = zip::ZipWriter::new(File::create(&jar).unwrap());
        let entries = [
            ("assets/minecraft/blockstates/stone.json", "{}"),
            ("data/minecraft/worldgen/biome/plains.json", "{}"),
            ("data/minecraft/timeline/day.json", "{}"),
            ("data/minecraft/tags/timeline/in_overworld.json", "{}"),
            ("data/minecraft/dimension_type/overworld.json", "{}"),
            ("data/minecraft/tags/block/leaves.json", "{}"),
            ("data/minecraft/recipe/stick.json", "x"),
            ("net/minecraft/Main.class", "x"),
            ("../assets/minecraft/escape.json", "x"),
        ];
        for (name, body) in entries {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        let out = dir.join("out");
        assert_eq!(unpack(&jar, &out).unwrap(), 6);
        assert!(out.join(BIOMES).join("plains.json").is_file() && !out.join("data/minecraft/recipe").exists());
        assert!(out.join(BLOCK_TAGS).join("leaves.json").is_file());
        assert_eq!(std::fs::read_to_string(out.join("assets/minecraft/blockstates/stone.json")).unwrap(), "{}");
        assert!(!dir.join("assets").exists() && !out.join("net").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
