//! The jar's banner textures (`textures/entity/banner/*.png`: the model's `banner_base`, the cloth
//! `base` and one image per pattern), copied into a Java look's files beside the Bedrock pack's
//! `banner_*.tga`; acacia-render's `banner::compose` prefers them when present.

use std::path::Path;

use crate::download::Error;
use crate::ui::copy_tree;

const COPIED: &str = "textures/entity/banner";

pub fn copy(assets: &Path, out: &Path) -> Result<(), Error> {
    copy_tree(&assets.join(COPIED), &out.join(COPIED)).map_err(|e| format!("{}: {e}", assets.join(COPIED).display()))?;
    Ok(())
}
