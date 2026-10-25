//! Java's entity models, which the game builds in code: the exporter `EntityModelExporter`
//! (exporter.rs) writes every baked model layer, which goes into the look as
//! `java/entity_models.json` beside the jar's entity textures (`java/textures/entity`), equipment
//! assets (`java/equipment`) and armour trims (`java/trims`: materials and patterns; the
//! unpaletted trim textures; trims.rs bakes the paletted ones). Without a JDK the look keeps
//! Bedrock's entity geometry.

use std::path::Path;

use serde_json::Value;

use crate::download::Error;
use crate::exporter::Exporter;
use crate::jar::Jar;
use crate::ui::copy_tree;

pub const MODELS: &str = "java/entity_models.json";
const TEXTURES: &str = "textures/entity";

/// Copies the models, textures, equipment and trims into `out`; the number of layers.
pub fn bake(exporter: &Exporter, jar: &Jar, out: &Path) -> Result<usize, Error> {
    let models = exporter.output("EntityModelExporter")?;
    let layers = serde_json::from_slice::<serde_json::Map<String, Value>>(&std::fs::read(&models)?)?.len();
    let to = out.join(MODELS);
    std::fs::create_dir_all(to.parent().expect("a file in a directory"))?;
    std::fs::copy(&models, to)?;
    let java = out.join("java");
    copy_tree(&jar.assets.join(TEXTURES), &java.join(TEXTURES))?;
    // Read by acacia-render entity/java/equipment.rs and trims.rs.
    copy_tree(&jar.assets.join("equipment"), &java.join("equipment"))?;
    copy_tree(&jar.assets.join("textures/trims/entity"), &java.join("textures/trims/entity"))?;
    copy_tree(&jar.data.join("trim_material"), &java.join("trims/material"))?;
    copy_tree(&jar.data.join("trim_pattern"), &java.join("trims/pattern"))?;
    Ok(layers)
}
