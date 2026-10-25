//! Java's entity models, which the game builds in code: the exporter `EntityModelExporter`
//! (exporter.rs) writes every baked model layer, which goes into the look as
//! `java/entity_models.json` beside the jar's entity textures (`java/textures/entity`) and
//! equipment assets (`java/equipment`). Without a
//! JDK the look keeps Bedrock's entity geometry.

use std::path::Path;

use serde_json::Value;

use crate::download::Error;
use crate::exporter::Exporter;
use crate::ui::copy_tree;

pub const MODELS: &str = "java/entity_models.json";
const TEXTURES: &str = "textures/entity";

/// Copies the models and the textures into `out`; the number of layers.
pub fn bake(exporter: &Exporter, assets: &Path, out: &Path) -> Result<usize, Error> {
    let models = exporter.output("EntityModelExporter")?;
    let layers = serde_json::from_slice::<serde_json::Map<String, Value>>(&std::fs::read(&models)?)?.len();
    let to = out.join(MODELS);
    std::fs::create_dir_all(to.parent().expect("a file in a directory"))?;
    std::fs::copy(&models, to)?;
    copy_tree(&assets.join(TEXTURES), &out.join("java").join(TEXTURES))?;
    // Equipment assets: which textures armour draws (acacia-render entity/java/equipment.rs).
    copy_tree(&assets.join("equipment"), &out.join("java/equipment"))?;
    Ok(layers)
}
