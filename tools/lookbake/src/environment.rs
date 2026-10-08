//! The jar's weather, cloud and underwater textures, copied into a Java look's files: `rain.png`
//! and `snow.png` (which Bedrock folds into one `weather.png`), Java's own `clouds.png`, replacing
//! the pack's, and `underwater.png` (Bedrock has no underwater overlay).

use std::path::Path;

use crate::download::Error;
use crate::ui::copy_tree;

const COPIED: [&str; 4] = [
    "textures/environment/rain.png",
    "textures/environment/snow.png",
    "textures/environment/clouds.png",
    "textures/misc/underwater.png",
];

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    for name in COPIED {
        copy_tree(&assets.join(name), &out.join(name)).map_err(|e| format!("{}: {e}", assets.join(name).display()))?;
    }
    Ok(())
}
