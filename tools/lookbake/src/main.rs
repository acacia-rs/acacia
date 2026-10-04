//! Bakes look packs for the renderer (docs/java-look.md). Directories default to: the Bedrock
//! pack `$ACACIA_ASSETS`, else assets/vanilla (tools/fetch-vanilla-pack.sh); Java assets
//! `$ACACIA_JAVA_ASSETS`, else assets/java; looks `$ACACIA_LOOKS`, else assets/looks.
//! - `lookbake bedrock [pack dir] [out dir]`: the Bedrock look.
//! - `lookbake java [pack dir] [java dir] [out dir]`: the Java look (java.rs), fetching what it needs.
//! - `lookbake fetch-java [java dir]`: downloads the pinned Java client jar, unpacks its assets
//!   (jar.rs) and downloads the Bedrock-to-Java block table (mapping.rs).
//! - `lookbake java-report [java dir]`: the same, then how many Bedrock block states reach a Java model.

mod blockstate;
mod download;
mod jar;
mod java;
mod mapping;
mod model;
mod report;
mod textures;

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::{Look, LookPack};

use download::Error;

const USAGE: &str = "usage: lookbake bedrock [pack dir] [out dir] | java [pack dir] [java dir] [out dir] | fetch-java [java dir] | java-report [java dir]";

fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    let mut dir = |default: fn() -> PathBuf| args.next().map_or_else(default, PathBuf::from);
    match command.as_deref() {
        Some("bedrock") => {
            let (pack, out) = (Pack::load(&dir(Pack::default_dir))?, dir(|| LookPack::default_dir("bedrock")));
            let (look, report) = LookPack::bake_bedrock(&pack, Look::BEDROCK);
            look.save(&out)?;
            println!("{}; {} blocks without textures, {} images missing", summary(&look, &out), report.blocks_without_textures.len(), report.missing_images.len());
        }
        Some("java") => {
            let (pack, java, out) = (Pack::load(&dir(Pack::default_dir))?, dir(jar::default_dir), dir(|| LookPack::default_dir("java")));
            let (assets, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let (look, report) = java::bake_look(&pack, &assets, &mapping);
            look.save(&out)?;
            println!("{}", summary(&look, &out));
            println!("  Java: {} states as cubes, {} as models; kept from Bedrock: {:?}", report.cubes, report.models, report.kept);
            println!("  textures missing: {:?}; models invalid: {:?}", report.missing_textures, report.invalid_models);
        }
        Some(command @ ("fetch-java" | "java-report")) => {
            let java = dir(jar::default_dir);
            let (assets, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            println!("Java {} assets: {}", jar::VERSION, assets.display());
            if command == "java-report" {
                report::print(&assets, &mapping)?;
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn summary(look: &LookPack, out: &std::path::Path) -> String {
    let (states, blocks) = look.counts();
    format!("{}: {states} states, {blocks} distinct blocks, {} textures ({} animated)", out.display(), look.atlas.layers.len(), look.atlas.animations.len())
}
