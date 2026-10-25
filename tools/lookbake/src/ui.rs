//! Everything the Java GUI draws from, copied into a Java look's files in the jar's own layout:
//! the GUI sprites with their `.mcmeta` scaling and sheets (`textures/gui`), the camera overlays
//! (`textures/misc`), effect icons, the fonts and their definitions, the nether portal's frames, and
//! the translations the screens' text comes from (`lang`) and the credits roll's texts (`texts`).
//! Read by `acacia_ui::theme::java`, `acacia_ui::java::paint::assets` and `acacia_ui::java::lang`.

use std::path::Path;

use crate::download::{Error, get, pinned};

const COPIED: [&str; 8] =
    ["textures/gui", "textures/misc", "textures/mob_effect", "textures/font", "font", "textures/block/nether_portal.png", "lang", "texts"];

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    for name in COPIED {
        copy_tree(&assets.join(name), &out.join(name)).map_err(|e| format!("{}: {e}", assets.join(name).display()))?;
    }
    Ok(())
}

/// The fonts' files only Mojang's asset index has (Unifont's zips and the definitions naming them,
/// `font/include/unifont.json` replacing the jar's empty one), saved under `out/font` as the
/// client's resources layer them; returns how many. Nothing here is redistributed.
pub fn fetch_fonts(out: &Path) -> Result<usize, Error> {
    let package = crate::jar::version_package()?;
    let index_url = package["assetIndex"]["url"].as_str().ok_or("version has no asset index")?;
    let index: serde_json::Value = serde_json::from_slice(&get(index_url)?)?;
    let objects = index["objects"].as_object().ok_or("asset index has no objects")?;
    let mut count = 0;
    for (key, object) in objects {
        let Some(path) = key.strip_prefix("minecraft/font/") else { continue };
        let hash = object["hash"].as_str().filter(|h| h.len() > 2).ok_or_else(|| format!("{key} has no hash"))?;
        pinned(&out.join("font").join(path), hash, || get(&format!("{}/{}/{hash}", crate::sounds::OBJECTS, &hash[..2])))?;
        count += 1;
    }
    Ok(count)
}

pub(crate) fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to).map(|_| ())
}
