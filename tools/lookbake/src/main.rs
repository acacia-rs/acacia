//! Bakes look packs for the renderer (docs/java-look.md). Directories default to: the Bedrock
//! pack `$ACACIA_ASSETS`, else assets/vanilla (tools/fetch-vanilla-pack.sh); Java assets
//! `$ACACIA_JAVA_ASSETS`, else assets/java; looks `$ACACIA_LOOKS`, else assets/looks.
//! - `lookbake bedrock [pack dir] [out dir]`: the Bedrock look.
//! - `lookbake java [pack dir] [java dir] [out dir]`: the Java look (java.rs), fetching what it needs.
//! - `lookbake fetch-java [java dir]`: downloads the pinned Java client jar, unpacks its assets
//!   (jar.rs) and downloads the Bedrock-to-Java block table (mapping.rs).
//! - `lookbake java-report [java dir]`: the same, then how many Bedrock block states reach a Java model.
//! - `lookbake java-entities [java dir] [out dir]`: only the Java entity models and textures
//!   (entities.rs; needs a JDK 25).
//! - `lookbake java-check <quads.json.gz> [java dir]`: our bake of every Java block state against a
//!   dump of the game's own (check.rs); tools/parity's harness `bake` makes the dump. Exits nonzero
//!   on any divergence.

mod banners;
mod biomes;
mod block;
mod blockstate;
mod check;
mod download;
mod entities;
mod environment;
mod jar;
mod java;
mod mapping;
mod model;
mod offset;
mod particles;
mod report;
mod shade;
mod sounds;
mod textures;
mod tint;
mod ui;

use std::path::PathBuf;

use acacia_render::assets::Pack;
use acacia_render::{Look, LookPack};

use download::Error;

const USAGE: &str = "usage: lookbake bedrock [pack dir] [out dir] | java [pack dir] [java dir] [out dir] | fetch-java [java dir] | java-entities [java dir] [out dir] | java-report [java dir] | java-check <quads.json.gz> [java dir]";

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
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let (look, report) = java::bake_look(&pack, &jar.assets, &mapping);
            look.save(&out)?;
            biomes::write(&biomes::java(&jar.biomes, &biomes::Colormaps::load(&jar.assets)?)?, &out)?;
            ui::copy(&jar.assets, &out)?;
            environment::copy(&jar.assets, &out)?;
            particles::copy(&jar.assets, &out)?;
            banners::copy(&jar.assets, &out)?;
            if let Some(layers) = entities::bake(&java, &jar.assets, &out)? {
                println!("  Java entity models: {layers} layers");
            }
            println!("  Java sounds: {} files", sounds::fetch(&out)?);
            println!("{}", summary(&look, &out));
            println!("  Java: {} states as cubes, {} as models, {} liquids; kept from Bedrock: {:?}", report.cubes, report.models, report.liquids, report.kept);
            println!("  textures missing: {:?}; models invalid: {:?}", report.missing_textures, report.invalid_models);
        }
        Some("java-entities") => {
            let (java, out) = (dir(jar::default_dir), dir(|| LookPack::default_dir("java")));
            let layers = entities::bake(&java, &jar::fetch(&java)?.assets, &out)?.ok_or("no JDK 25")?;
            println!("{}: {layers} entity model layers", out.join(entities::MODELS).display());
        }
        Some(command @ ("fetch-java" | "java-report")) => {
            let java = dir(jar::default_dir);
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            println!("Java {} assets: {}", jar::VERSION, jar.assets.display());
            if command == "java-report" {
                report::print(&jar.assets, &mapping)?;
            }
        }
        Some("java-check") => {
            let truth = Some(dir(PathBuf::new)).filter(|p| !p.as_os_str().is_empty()).ok_or(USAGE)?;
            let java = dir(jar::default_dir);
            let (jar, mapping) = (jar::fetch(&java)?, mapping::Mapping::fetch(&java)?);
            let outcome = check::print(&jar.assets, &biomes::java(&jar.biomes, &biomes::Colormaps::load(&jar.assets)?)?, &mapping, &truth)?;
            outcome.print_for_parity();
            if !outcome.divergences.is_empty() {
                return Err(format!("{} divergences from the game", outcome.divergences.len()).into());
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
