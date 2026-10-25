//! Everything the Java GUI draws from, copied into a Java look's files in the jar's own layout:
//! the GUI sprites with their `.mcmeta` scaling and sheets (`textures/gui`), the camera overlays
//! (`textures/misc`), effect icons, the fonts and their definitions, and the nether portal's frames.
//! Read by `acacia_ui::theme::java` and `acacia_ui::java::paint::assets`.

use std::path::Path;

use crate::download::Error;

const COPIED: [&str; 6] = ["textures/gui", "textures/misc", "textures/mob_effect", "textures/font", "font", "textures/block/nether_portal.png"];

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    for name in COPIED {
        copy_tree(&assets.join(name), &out.join(name)).map_err(|e| format!("{}: {e}", assets.join(name).display()))?;
    }
    Ok(())
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
