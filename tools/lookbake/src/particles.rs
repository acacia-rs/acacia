//! The jar's particle textures (`textures/particle/*.png`, one file per sprite), copied into a Java
//! look's files beside the Bedrock pack's `particles.png` sheet; acacia-render's
//! `particles::sheet` prefers them when present.

use std::path::Path;

use crate::download::Error;
use crate::ui::copy_tree;

const COPIED: &str = "textures/particle";

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    copy_tree(&assets.join(COPIED), &out.join(COPIED)).map_err(|e| format!("{}: {e}", assets.join(COPIED).display()))?;
    Ok(())
}
