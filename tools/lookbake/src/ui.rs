//! The jar's HUD sprites and ASCII font, copied into a Java look's files for acacia-ui's Java
//! theme (`acacia_ui::theme::java`), in the jar's own layout.

use std::path::Path;

use crate::download::Error;

const COPIED: [&str; 3] = ["textures/gui/sprites/hud", "textures/font/ascii.png", "font/include/default.json"];

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    for name in COPIED {
        copy_tree(&assets.join(name), &out.join(name)).map_err(|e| format!("{}: {e}", assets.join(name).display()))?;
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
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
